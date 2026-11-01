use async_trait::async_trait;
use futures::StreamExt;
use lyra_core::{
    ColumnMetadata, ColumnSchema, CoreError, DatabaseConnection, QueryResult, Row as LyraRow,
    TableSchema, Value,
};
use mongodb::bson::{doc, Bson, Document};
use mongodb::{options::ClientOptions, Client, Database};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

pub struct MongoConnection {
    client: Client,
    db: Database,
    url: String,
    db_name: String,
}

impl MongoConnection {
    /// Conecta a un clúster o servidor MongoDB mediante URI estándar.
    ///
    /// Ejemplo: `mongodb://localhost:27017/mi_base` o `mongodb://user:pass@host:27017`
    pub async fn connect(url: &str) -> Result<Self, CoreError> {
        let mut options = ClientOptions::parse(url)
            .await
            .map_err(|e| CoreError::ConnectionFailed(format!("URI de MongoDB inválida: {e}")))?;

        options.app_name = Some("Lyra".to_string());

        let client = Client::with_options(options).map_err(|e| {
            CoreError::ConnectionFailed(format!("Fallo inicializando cliente MongoDB: {e}"))
        })?;

        // Extraer el nombre de la base de datos de la URI o usar 'admin' / 'test' por defecto
        let db_name = extract_db_name(url).unwrap_or_else(|| "test".to_string());
        let db = client.database(&db_name);

        Ok(Self {
            client,
            db,
            url: url.to_string(),
            db_name,
        })
    }

    pub fn database_name(&self) -> &str {
        &self.db_name
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn database(&self) -> &Database {
        &self.db
    }
}

#[async_trait]
impl DatabaseConnection for MongoConnection {
    fn engine_name(&self) -> &'static str {
        "MongoDB"
    }

    async fn ping(&self) -> Result<(), CoreError> {
        self.db
            .run_command(doc! { "ping": 1 })
            .await
            .map_err(|e| CoreError::ConnectionFailed(format!("Fallo de ping en MongoDB: {e}")))?;
        Ok(())
    }

    async fn execute(&self, query: &str) -> Result<QueryResult, CoreError> {
        let start = Instant::now();
        let trimmed = query.trim();

        if trimmed.starts_with('{') {
            // 1. Comando JSON/BSON crudo (ej: {"find": "usuarios", "limit": 20})
            let command_doc: Document = serde_json::from_str(trimmed)
                .map_err(|e| CoreError::QueryError(format!("Comando JSON/BSON inválido: {e}")))?;

            let response = self
                .db
                .run_command(command_doc)
                .await
                .map_err(|e| CoreError::QueryError(format!("Error en comando MongoDB: {e}")))?;

            let execution_time_ms = start.elapsed().as_millis();

            // Si es un comando 'find' o 'aggregate', la respuesta contiene un cursor con los documentos
            if let Ok(cursor_doc) = response.get_document("cursor") {
                if let Ok(first_batch) = cursor_doc.get_array("firstBatch") {
                    let docs: Vec<Document> = first_batch
                        .iter()
                        .filter_map(|b| b.as_document().cloned())
                        .collect();
                    return Ok(documents_to_query_result(docs, execution_time_ms));
                }
            }

            // Para otros comandos (como count, ping, insert), devolver la respuesta como una fila única
            Ok(documents_to_query_result(vec![response], execution_time_ms))
        } else {
            // 2. Consulta shorthand tipo: `coleccion.find({...})` o simplemente `coleccion`
            let (coll_name, filter_doc) = parse_shorthand_query(trimmed)?;

            let coll = self.db.collection::<Document>(&coll_name);
            let mut cursor = coll.find(filter_doc).limit(100).await.map_err(|e| {
                CoreError::QueryError(format!("Error consultando colección {coll_name}: {e}"))
            })?;

            let mut docs = Vec::new();
            while let Some(doc_result) = cursor.next().await {
                let d = doc_result.map_err(|e| CoreError::QueryError(e.to_string()))?;
                docs.push(d);
            }

            let execution_time_ms = start.elapsed().as_millis();
            Ok(documents_to_query_result(docs, execution_time_ms))
        }
    }

    async fn list_tables(&self) -> Result<Vec<String>, CoreError> {
        let colls = self.db.list_collection_names().await.map_err(|e| {
            CoreError::QueryError(format!("Error listando colecciones en MongoDB: {e}"))
        })?;

        Ok(colls)
    }

    async fn describe_table(&self, collection: &str) -> Result<TableSchema, CoreError> {
        let coll = self.db.collection::<Document>(collection);

        // Inferencia de esquema por muestreo: tomar hasta 50 documentos representativos
        let mut cursor = coll.find(doc! {}).limit(50).await.map_err(|e| {
            CoreError::QueryError(format!("Error muestreando colección {collection}: {e}"))
        })?;

        let mut sample_count = 0;
        let mut field_occurrences: BTreeMap<String, usize> = BTreeMap::new();
        let mut field_types: BTreeMap<String, BTreeSet<&'static str>> = BTreeMap::new();

        while let Some(Ok(doc)) = cursor.next().await {
            sample_count += 1;
            for (key, val) in doc.iter() {
                *field_occurrences.entry(key.clone()).or_insert(0) += 1;
                field_types
                    .entry(key.clone())
                    .or_default()
                    .insert(bson_type_name(val));
            }
        }

        if sample_count == 0 {
            // Colección vacía o sin documentos todavía
            let default_cols = vec![ColumnSchema {
                name: "_id".to_string(),
                data_type: "ObjectId".to_string(),
                is_nullable: false,
                is_primary_key: true,
                default_value: None,
            }];

            return Ok(TableSchema {
                name: collection.to_string(),
                schema_name: Some(self.db_name.clone()),
                columns: default_cols,
                foreign_keys: Vec::new(),
                estimated_row_count: Some(0),
            });
        }

        // Construir ColumnSchema para cada campo inferido
        let mut columns = Vec::new();
        for (field, types) in field_types {
            let is_primary_key = field == "_id";
            let occurrences = field_occurrences.get(&field).copied().unwrap_or(0);
            let is_nullable = occurrences < sample_count || types.contains("Null");

            let type_summary = types.into_iter().collect::<Vec<_>>().join(" | ");

            columns.push(ColumnSchema {
                name: field,
                data_type: type_summary,
                is_nullable,
                is_primary_key,
                default_value: None,
            });
        }

        // Asegurar que `_id` quede al inicio
        columns.sort_by(|a, b| {
            if a.name == "_id" {
                std::cmp::Ordering::Less
            } else if b.name == "_id" {
                std::cmp::Ordering::Greater
            } else {
                a.name.cmp(&b.name)
            }
        });

        let estimated_count = coll.estimated_document_count().await.ok();

        Ok(TableSchema {
            name: collection.to_string(),
            schema_name: Some(self.db_name.clone()),
            columns,
            foreign_keys: Vec::new(), // En NoSQL no hay FK nativas; se definirán relaciones lógicas
            estimated_row_count: estimated_count,
        })
    }

    async fn close(&self) -> Result<(), CoreError> {
        // En mongodb-rs el cliente se cierra automáticamente al descartarse
        Ok(())
    }
}

/// Convierte una lista de documentos BSON a un QueryResult canónico con columnas normalizadas.
fn documents_to_query_result(docs: Vec<Document>, execution_time_ms: u128) -> QueryResult {
    if docs.is_empty() {
        return QueryResult::empty(execution_time_ms);
    }

    // 1. Recolectar la unión de todas las claves presentes en los documentos
    let mut key_set = BTreeSet::new();
    for d in &docs {
        for k in d.keys() {
            key_set.insert(k.clone());
        }
    }

    // Ordenar con `_id` al frente
    let mut keys: Vec<String> = key_set.into_iter().collect();
    keys.sort_by(|a, b| {
        if a == "_id" {
            std::cmp::Ordering::Less
        } else if b == "_id" {
            std::cmp::Ordering::Greater
        } else {
            a.cmp(b)
        }
    });

    let columns: Vec<ColumnMetadata> = keys
        .iter()
        .map(|k| ColumnMetadata::new(k, "BSON", true))
        .collect();

    // 2. Mapear cada documento a una fila de Values
    let mut rows = Vec::with_capacity(docs.len());
    for d in docs {
        let mut row_values = Vec::with_capacity(keys.len());
        for k in &keys {
            match d.get(k) {
                Some(bson) => row_values.push(bson_to_canonical_value(bson)),
                None => row_values.push(Value::Null),
            }
        }
        rows.push(LyraRow::new(row_values));
    }

    QueryResult {
        columns,
        rows,
        rows_affected: 0,
        execution_time_ms,
    }
}

/// Mapea un tipo Bson de MongoDB al enum canónico Value de Lyra.
fn bson_to_canonical_value(b: &Bson) -> Value {
    match b {
        Bson::Null => Value::Null,
        Bson::Boolean(v) => Value::Boolean(*v),
        Bson::Int32(v) => Value::Integer(*v as i64),
        Bson::Int64(v) => Value::Integer(*v),
        Bson::Double(v) => Value::Real(*v),
        Bson::String(v) => Value::Text(v.clone()),
        Bson::ObjectId(oid) => Value::Text(oid.to_hex()),
        Bson::DateTime(dt) => Value::DateTime(dt.to_string()),
        Bson::Binary(bin) => Value::Blob(bin.bytes.clone()),
        Bson::Document(d) => {
            let json_val = serde_json::to_value(d).unwrap_or(serde_json::Value::Null);
            Value::Json(json_val)
        }
        Bson::Array(arr) => {
            let json_val = serde_json::to_value(arr).unwrap_or(serde_json::Value::Null);
            Value::Json(json_val)
        }
        other => Value::Text(other.to_string()),
    }
}

fn bson_type_name(b: &Bson) -> &'static str {
    match b {
        Bson::Null => "Null",
        Bson::Boolean(_) => "Boolean",
        Bson::Int32(_) => "Int32",
        Bson::Int64(_) => "Int64",
        Bson::Double(_) => "Double",
        Bson::String(_) => "String",
        Bson::ObjectId(_) => "ObjectId",
        Bson::DateTime(_) => "DateTime",
        Bson::Binary(_) => "Binary",
        Bson::Array(_) => "Array",
        Bson::Document(_) => "Document",
        _ => "Other",
    }
}

fn extract_db_name(url: &str) -> Option<String> {
    let without_prefix = url.strip_prefix("mongodb://")?;
    let path_start = without_prefix.find('/')?;
    let path = &without_prefix[path_start + 1..];
    let query_start = path.find('?').unwrap_or(path.len());
    let db = &path[..query_start];
    if db.is_empty() {
        None
    } else {
        Some(db.to_string())
    }
}

fn parse_shorthand_query(query: &str) -> Result<(String, Document), CoreError> {
    if let Some((coll, rest)) = query.split_once(".find(") {
        let filter_str = rest.trim_end_matches(')').trim();
        let filter = if filter_str.is_empty() {
            doc! {}
        } else {
            serde_json::from_str(filter_str)
                .map_err(|e| CoreError::QueryError(format!("Filtro de búsqueda inválido: {e}")))?
        };
        Ok((coll.trim().to_string(), filter))
    } else {
        // Asumir que se pasa únicamente el nombre de la colección: `pedidos`
        Ok((query.trim().to_string(), doc! {}))
    }
}
