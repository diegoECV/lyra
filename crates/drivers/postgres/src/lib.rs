use async_trait::async_trait;
use lyra_core::{
    ColumnMetadata, ColumnSchema, CoreError, DatabaseConnection, ForeignKeySchema, QueryResult,
    Row as LyraRow, TableSchema, Value,
};
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions, PgRow};
use sqlx::{Column, Row, TypeInfo, ValueRef};
use std::str::FromStr;
use std::time::Instant;

pub struct PostgresConnection {
    pool: PgPool,
    url: String,
}

impl PostgresConnection {
    /// Conecta a una base de datos PostgreSQL mediante una cadena de conexión estándar.
    ///
    /// Ejemplo: `postgres://usuario:password@localhost:5432/mi_base`
    pub async fn connect(url: &str) -> Result<Self, CoreError> {
        let options = PgConnectOptions::from_str(url)
            .map_err(|e| CoreError::ConnectionFailed(format!("URL de PostgreSQL inválida: {e}")))?;

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(|e| {
                CoreError::ConnectionFailed(format!("No se pudo conectar a PostgreSQL: {e}"))
            })?;

        Ok(Self {
            pool,
            url: url.to_string(),
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}

#[async_trait]
impl DatabaseConnection for PostgresConnection {
    fn engine_name(&self) -> &'static str {
        "PostgreSQL"
    }

    async fn ping(&self) -> Result<(), CoreError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e| {
                CoreError::ConnectionFailed(format!("Fallo de ping en PostgreSQL: {e}"))
            })?;
        Ok(())
    }

    async fn execute(&self, query: &str) -> Result<QueryResult, CoreError> {
        let start = Instant::now();
        let trimmed = query.trim_start().to_uppercase();

        let is_query = trimmed.starts_with("SELECT")
            || trimmed.starts_with("WITH")
            || trimmed.starts_with("EXPLAIN")
            || trimmed.starts_with("SHOW")
            || trimmed.contains("RETURNING");

        if is_query {
            let pg_rows: Vec<PgRow> = sqlx::query(query)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| CoreError::QueryError(format!("Error en consulta PostgreSQL: {e}")))?;

            let execution_time_ms = start.elapsed().as_millis();

            if pg_rows.is_empty() {
                return Ok(QueryResult::empty(execution_time_ms));
            }

            // Metadatos de columnas
            let first_row = &pg_rows[0];
            let columns: Vec<ColumnMetadata> = first_row
                .columns()
                .iter()
                .map(|c| ColumnMetadata::new(c.name(), c.type_info().name(), true))
                .collect();

            // Mapeo dinámico de filas
            let mut rows = Vec::with_capacity(pg_rows.len());
            for r in pg_rows {
                let mut values = Vec::with_capacity(columns.len());
                for (i, col) in columns.iter().enumerate() {
                    values.push(extract_pg_value(&r, i, &col.type_name));
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
            // Operación DDL o DML (CREATE, INSERT, UPDATE, DELETE, etc.)
            let result = sqlx::query(query).execute(&self.pool).await.map_err(|e| {
                CoreError::QueryError(format!("Error ejecutando sentencia en PostgreSQL: {e}"))
            })?;

            let execution_time_ms = start.elapsed().as_millis();
            Ok(QueryResult::with_affected(
                result.rows_affected(),
                execution_time_ms,
            ))
        }
    }

    async fn list_tables(&self) -> Result<Vec<String>, CoreError> {
        let sql = "
            SELECT table_name 
            FROM information_schema.tables 
            WHERE table_schema = 'public' 
              AND table_type = 'BASE TABLE'
            ORDER BY table_name;
        ";

        let rows = sqlx::query_as::<_, (String,)>(sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                CoreError::QueryError(format!("Error listando tablas en PostgreSQL: {e}"))
            })?;

        Ok(rows.into_iter().map(|(name,)| name).collect())
    }

    async fn describe_table(&self, table: &str) -> Result<TableSchema, CoreError> {
        // 1. Columnas desde information_schema
        let cols_sql = "
            SELECT column_name, data_type, is_nullable, column_default
            FROM information_schema.columns
            WHERE table_schema = 'public' AND table_name = $1
            ORDER BY ordinal_position;
        ";

        let col_rows = sqlx::query_as::<_, (String, String, String, Option<String>)>(cols_sql)
            .bind(table)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                CoreError::QueryError(format!("Error inspeccionando columnas de {table}: {e}"))
            })?;

        if col_rows.is_empty() {
            return Err(CoreError::NotFound(table.to_string()));
        }

        // 2. Claves primarias
        let pk_sql = "
            SELECT kcu.column_name
            FROM information_schema.table_constraints tc
            JOIN information_schema.key_column_usage kcu
              ON tc.constraint_name = kcu.constraint_name
             AND tc.table_schema = kcu.table_schema
            WHERE tc.constraint_type = 'PRIMARY KEY'
              AND tc.table_schema = 'public'
              AND tc.table_name = $1;
        ";

        let pk_rows = sqlx::query_as::<_, (String,)>(pk_sql)
            .bind(table)
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();

        let pk_set: std::collections::HashSet<String> = pk_rows.into_iter().map(|(c,)| c).collect();

        let columns = col_rows
            .into_iter()
            .map(|(name, data_type, is_nullable, default_val)| {
                let is_primary_key = pk_set.contains(&name);
                ColumnSchema {
                    name,
                    data_type,
                    is_nullable: is_nullable == "YES",
                    is_primary_key,
                    default_value: default_val,
                }
            })
            .collect();

        // 3. Claves foráneas
        let fk_sql = "
            SELECT
                tc.constraint_name,
                kcu.column_name as from_column,
                ccu.table_name AS to_table,
                ccu.column_name AS to_column
            FROM information_schema.table_constraints AS tc
            JOIN information_schema.key_column_usage AS kcu
              ON tc.constraint_name = kcu.constraint_name
             AND tc.table_schema = kcu.table_schema
            JOIN information_schema.constraint_column_usage AS ccu
              ON ccu.constraint_name = tc.constraint_name
            WHERE tc.constraint_type = 'FOREIGN KEY'
              AND tc.table_schema = 'public'
              AND tc.table_name = $1;
        ";

        let fk_rows = sqlx::query_as::<_, (String, String, String, String)>(fk_sql)
            .bind(table)
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();

        let foreign_keys = fk_rows
            .into_iter()
            .map(
                |(constraint, from_col, to_table, to_col)| ForeignKeySchema {
                    constraint_name: Some(constraint),
                    from_column: from_col,
                    to_table,
                    to_column: to_col,
                    is_logical: false,
                },
            )
            .collect();

        // 4. Estimación de registros con pg_class
        let est_sql = "SELECT reltuples::bigint FROM pg_class WHERE relname = $1;";
        let estimated_row_count = sqlx::query_as::<_, (i64,)>(est_sql)
            .bind(table)
            .fetch_one(&self.pool)
            .await
            .map(|(c,)| c.max(0) as u64)
            .ok();

        Ok(TableSchema {
            name: table.to_string(),
            schema_name: Some("public".to_string()),
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

/// Extrae dinámicamente un valor de PostgreSQL y lo mapea al tipo canónico Value.
fn extract_pg_value(row: &PgRow, index: usize, type_name: &str) -> Value {
    if let Ok(raw_ref) = row.try_get_raw(index) {
        if raw_ref.is_null() {
            return Value::Null;
        }
    }

    let upper = type_name.to_uppercase();

    if upper.contains("BOOL") {
        if let Ok(v) = row.try_get::<bool, _>(index) {
            return Value::Boolean(v);
        }
    } else if upper.contains("INT2") || upper.contains("SMALLINT") {
        if let Ok(v) = row.try_get::<i16, _>(index) {
            return Value::Integer(v as i64);
        }
    } else if upper.contains("INT4") || upper.contains("INT") || upper.contains("SERIAL") {
        if let Ok(v) = row.try_get::<i32, _>(index) {
            return Value::Integer(v as i64);
        }
    } else if upper.contains("INT8") || upper.contains("BIGINT") {
        if let Ok(v) = row.try_get::<i64, _>(index) {
            return Value::Integer(v);
        }
    } else if upper.contains("FLOAT4") {
        if let Ok(v) = row.try_get::<f32, _>(index) {
            return Value::Real(v as f64);
        }
    } else if upper.contains("FLOAT8") || upper.contains("DOUBLE") {
        if let Ok(v) = row.try_get::<f64, _>(index) {
            return Value::Real(v);
        }
    } else if upper.contains("BYTEA") {
        if let Ok(v) = row.try_get::<Vec<u8>, _>(index) {
            return Value::Blob(v);
        }
    } else if upper.contains("JSON") {
        if let Ok(v) = row.try_get::<serde_json::Value, _>(index) {
            return Value::Json(v);
        }
    }

    // Fallbacks para cadenas de texto, fechas, UUIDs y tipos personalizados
    if let Ok(s) = row.try_get::<String, _>(index) {
        Value::Text(s)
    } else if let Ok(i) = row.try_get::<i64, _>(index) {
        Value::Integer(i)
    } else if let Ok(f) = row.try_get::<f64, _>(index) {
        Value::Real(f)
    } else {
        Value::Null
    }
}
