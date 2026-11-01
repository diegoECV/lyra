use async_trait::async_trait;
use lyra_core::{
    ColumnMetadata, ColumnSchema, CoreError, DatabaseConnection, ForeignKeySchema, QueryResult,
    Row as LyraRow, TableSchema, Value,
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions, SqliteRow};
use sqlx::{Column, Row, TypeInfo, ValueRef};
use std::str::FromStr;
use std::time::Instant;

pub struct SqliteConnection {
    pool: SqlitePool,
    url: String,
}

impl SqliteConnection {
    /// Conectar a una base SQLite (en memoria o en disco).
    ///
    /// Ejemplos de URL:
    /// - `sqlite::memory:` (base de datos volátil en memoria)
    /// - `sqlite://test.db` o `sqlite:/ruta/a/la/base.db?mode=rwc`
    pub async fn connect(url: &str) -> Result<Self, CoreError> {
        let options = SqliteConnectOptions::from_str(url)
            .map_err(|e| CoreError::ConnectionFailed(format!("URL de SQLite inválida: {e}")))?
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(|e| {
                CoreError::ConnectionFailed(format!("No se pudo conectar a SQLite: {e}"))
            })?;

        Ok(Self {
            pool,
            url: url.to_string(),
        })
    }

    /// Crea una base de datos SQLite en memoria rápida para pruebas.
    pub async fn in_memory() -> Result<Self, CoreError> {
        Self::connect("sqlite::memory:").await
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}

#[async_trait]
impl DatabaseConnection for SqliteConnection {
    fn engine_name(&self) -> &'static str {
        "SQLite"
    }

    async fn ping(&self) -> Result<(), CoreError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e| CoreError::ConnectionFailed(e.to_string()))?;
        Ok(())
    }

    async fn execute(&self, query: &str) -> Result<QueryResult, CoreError> {
        let start = Instant::now();
        let trimmed = query.trim_start().to_uppercase();

        let is_query = trimmed.starts_with("SELECT")
            || trimmed.starts_with("PRAGMA")
            || trimmed.starts_with("WITH")
            || trimmed.starts_with("EXPLAIN")
            || trimmed.contains("RETURNING");

        if is_query {
            let sqlx_rows: Vec<SqliteRow> = sqlx::query(query)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| CoreError::QueryError(e.to_string()))?;

            let execution_time_ms = start.elapsed().as_millis();

            if sqlx_rows.is_empty() {
                // Obtener metadatos de columnas si es posible o devolver vacío
                return Ok(QueryResult::empty(execution_time_ms));
            }

            // Extraer metadatos de las columnas desde la primera fila
            let first_row = &sqlx_rows[0];
            let columns: Vec<ColumnMetadata> = first_row
                .columns()
                .iter()
                .map(|c| ColumnMetadata::new(c.name(), c.type_info().name(), true))
                .collect();

            // Mapear cada fila a tipos canónicos Value
            let mut rows = Vec::with_capacity(sqlx_rows.len());
            for r in sqlx_rows {
                let mut values = Vec::with_capacity(columns.len());
                for (i, col) in columns.iter().enumerate() {
                    values.push(extract_sqlite_value(&r, i, &col.type_name));
                }
                rows.push(LyraRow::new(values));
            }

            Ok(QueryResult {
                columns,
                rows,
                rows_affected: 0,
                execution_time_ms,
            })
        } else {
            // Operación DDL o DML (CREATE, INSERT, UPDATE, DELETE, DROP)
            let result = sqlx::query(query)
                .execute(&self.pool)
                .await
                .map_err(|e| CoreError::QueryError(e.to_string()))?;

            let execution_time_ms = start.elapsed().as_millis();
            Ok(QueryResult::with_affected(
                result.rows_affected(),
                execution_time_ms,
            ))
        }
    }

    async fn list_tables(&self) -> Result<Vec<String>, CoreError> {
        let sql = "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name;";
        let rows = sqlx::query_as::<_, (String,)>(sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| CoreError::QueryError(format!("Error listando tablas: {e}")))?;

        Ok(rows.into_iter().map(|(name,)| name).collect())
    }

    async fn describe_table(&self, table: &str) -> Result<TableSchema, CoreError> {
        // 1. Obtener columnas con PRAGMA table_info
        let pragma_cols_sql = format!("PRAGMA table_info(\"{table}\");");
        let col_rows = sqlx::query(&pragma_cols_sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                CoreError::QueryError(format!("Error inspeccionando columnas de {table}: {e}"))
            })?;

        if col_rows.is_empty() {
            return Err(CoreError::NotFound(table.to_string()));
        }

        let mut columns = Vec::new();
        for r in col_rows {
            let name: String = r.try_get("name").unwrap_or_default();
            let data_type: String = r.try_get("type").unwrap_or_else(|_| "TEXT".to_string());
            let notnull: i64 = r.try_get("notnull").unwrap_or(0);
            let pk: i64 = r.try_get("pk").unwrap_or(0);
            let default_val: Option<String> = r.try_get("dflt_value").ok();

            columns.push(ColumnSchema {
                name,
                data_type,
                is_nullable: notnull == 0,
                is_primary_key: pk > 0,
                default_value: default_val,
            });
        }

        // 2. Obtener claves foráneas con PRAGMA foreign_key_list
        let pragma_fk_sql = format!("PRAGMA foreign_key_list(\"{table}\");");
        let fk_rows = sqlx::query(&pragma_fk_sql)
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();

        let mut foreign_keys = Vec::new();
        for r in fk_rows {
            let to_table: String = r.try_get("table").unwrap_or_default();
            let from_col: String = r.try_get("from").unwrap_or_default();
            let to_col: String = r.try_get("to").unwrap_or_default();

            foreign_keys.push(ForeignKeySchema {
                constraint_name: None,
                from_column: from_col,
                to_table,
                to_column: to_col,
                is_logical: false,
            });
        }

        // 3. Estimar conteo de filas
        let count_sql = format!("SELECT COUNT(*) FROM \"{table}\";");
        let estimated_row_count = sqlx::query_as::<_, (i64,)>(&count_sql)
            .fetch_one(&self.pool)
            .await
            .map(|(c,)| c as u64)
            .ok();

        Ok(TableSchema {
            name: table.to_string(),
            schema_name: None,
            columns,
            foreign_keys,
            estimated_row_count,
        })
    }

    async fn close(&self) -> Result<(), CoreError> {
        self.pool.close().await;
        Ok(())
    }
}

/// Extrae dinámicamente el valor de una columna de SQLite mapeándolo al enum canónico Value.
fn extract_sqlite_value(row: &SqliteRow, index: usize, type_name: &str) -> Value {
    if let Ok(raw_ref) = row.try_get_raw(index) {
        if raw_ref.is_null() {
            return Value::Null;
        }
    }

    let upper_type = type_name.to_uppercase();

    if upper_type.contains("INT") {
        if let Ok(val) = row.try_get::<i64, _>(index) {
            return Value::Integer(val);
        }
    } else if upper_type.contains("REAL")
        || upper_type.contains("FLOA")
        || upper_type.contains("DOUB")
    {
        if let Ok(val) = row.try_get::<f64, _>(index) {
            return Value::Real(val);
        }
    } else if upper_type.contains("BOOL") {
        if let Ok(val) = row.try_get::<bool, _>(index) {
            return Value::Boolean(val);
        }
    } else if upper_type.contains("BLOB") {
        if let Ok(val) = row.try_get::<Vec<u8>, _>(index) {
            return Value::Blob(val);
        }
    }

    // Fallbacks para texto o representación general
    if let Ok(val) = row.try_get::<String, _>(index) {
        Value::Text(val)
    } else if let Ok(val) = row.try_get::<i64, _>(index) {
        Value::Integer(val)
    } else if let Ok(val) = row.try_get::<f64, _>(index) {
        Value::Real(val)
    } else {
        Value::Null
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_in_memory_lifecycle() {
        let conn = SqliteConnection::in_memory()
            .await
            .expect("Conexión en memoria");
        assert_eq!(conn.engine_name(), "SQLite");
        conn.ping().await.expect("Ping exitoso");

        // Crear tabla
        let ddl =
            "CREATE TABLE usuarios (id INTEGER PRIMARY KEY, nombre TEXT NOT NULL, saldo REAL);";
        let res = conn.execute(ddl).await.expect("Crear tabla");
        assert_eq!(res.rows_affected, 0);

        // Insertar datos
        let insert =
            "INSERT INTO usuarios (nombre, saldo) VALUES ('Diego', 150.50), ('Ana', 300.0);";
        let res = conn.execute(insert).await.expect("Insertar registros");
        assert_eq!(res.rows_affected, 2);

        // Listar tablas
        let tables = conn.list_tables().await.expect("Listar tablas");
        assert_eq!(tables, vec!["usuarios".to_string()]);

        // Inspeccionar esquema
        let schema = conn
            .describe_table("usuarios")
            .await
            .expect("Inspeccionar esquema");
        assert_eq!(schema.columns.len(), 3);
        assert_eq!(schema.columns[0].name, "id");
        assert!(schema.columns[0].is_primary_key);

        // Consultar
        let query = "SELECT id, nombre, saldo FROM usuarios ORDER BY id ASC;";
        let query_res = conn.execute(query).await.expect("Consultar datos");
        assert_eq!(query_res.columns.len(), 3);
        assert_eq!(query_res.rows.len(), 2);
        assert_eq!(
            query_res.rows[0].get(1),
            Some(&Value::Text("Diego".to_string()))
        );
        assert_eq!(query_res.rows[0].get(2), Some(&Value::Real(150.50)));
    }
}
