use async_trait::async_trait;
use lyra_core::{
    ColumnMetadata, ColumnSchema, CoreError, DatabaseConnection, ForeignKeySchema, QueryResult,
    Row as LyraRow, TableSchema, Value,
};
use sqlx::mysql::{MySqlConnectOptions, MySqlPool, MySqlPoolOptions, MySqlRow};
use sqlx::{Column, Row, TypeInfo, ValueRef};
use std::str::FromStr;
use std::time::Instant;

pub struct MySqlConnection {
    pool: MySqlPool,
    url: String,
}

impl MySqlConnection {
    /// Conecta a una base de datos MySQL o MariaDB mediante una URL estándar.
    ///
    /// Ejemplo: `mysql://usuario:password@localhost:3306/mi_base`
    pub async fn connect(url: &str) -> Result<Self, CoreError> {
        let options = MySqlConnectOptions::from_str(url)
            .map_err(|e| CoreError::ConnectionFailed(format!("URL de MySQL inválida: {e}")))?;

        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(|e| {
                CoreError::ConnectionFailed(format!("No se pudo conectar a MySQL / MariaDB: {e}"))
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
impl DatabaseConnection for MySqlConnection {
    fn engine_name(&self) -> &'static str {
        "MySQL / MariaDB"
    }

    async fn ping(&self) -> Result<(), CoreError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e| {
                CoreError::ConnectionFailed(format!("Fallo de ping en MySQL / MariaDB: {e}"))
            })?;
        Ok(())
    }

    async fn execute(&self, query: &str) -> Result<QueryResult, CoreError> {
        let start = Instant::now();
        let trimmed = query.trim_start().to_uppercase();

        let is_query = trimmed.starts_with("SELECT")
            || trimmed.starts_with("SHOW")
            || trimmed.starts_with("DESCRIBE")
            || trimmed.starts_with("DESC ")
            || trimmed.starts_with("EXPLAIN");

        if is_query {
            let mysql_rows: Vec<MySqlRow> = sqlx::query(query)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| CoreError::QueryError(format!("Error en consulta MySQL: {e}")))?;

            let execution_time_ms = start.elapsed().as_millis();

            if mysql_rows.is_empty() {
                return Ok(QueryResult::empty(execution_time_ms));
            }

            // Metadatos de columnas
            let first_row = &mysql_rows[0];
            let columns: Vec<ColumnMetadata> = first_row
                .columns()
                .iter()
                .map(|c| ColumnMetadata::new(c.name(), c.type_info().name(), true))
                .collect();

            // Mapeo dinámico de filas
            let mut rows = Vec::with_capacity(mysql_rows.len());
            for r in mysql_rows {
                let mut values = Vec::with_capacity(columns.len());
                for (i, col) in columns.iter().enumerate() {
                    values.push(extract_mysql_value(&r, i, &col.type_name));
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
            let res = sqlx::query(query).execute(&self.pool).await.map_err(|e| {
                CoreError::QueryError(format!("Error en comando DDL/DML de MySQL: {e}"))
            })?;

            let execution_time_ms = start.elapsed().as_millis();
            Ok(QueryResult::with_affected(
                res.rows_affected(),
                execution_time_ms,
            ))
        }
    }

    async fn list_tables(&self) -> Result<Vec<String>, CoreError> {
        let query = "SELECT table_name FROM information_schema.tables WHERE table_schema = DATABASE() AND table_type = 'BASE TABLE' ORDER BY table_name;";

        let rows: Vec<MySqlRow> = sqlx::query(query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| CoreError::QueryError(format!("Error al listar tablas en MySQL: {e}")))?;

        let tables: Vec<String> = rows
            .iter()
            .filter_map(|r| r.try_get::<String, _>(0).ok())
            .collect();

        Ok(tables)
    }

    async fn describe_table(&self, table: &str) -> Result<TableSchema, CoreError> {
        // 1. Obtener columnas del esquema
        let col_query = "SELECT column_name, data_type, is_nullable, column_key, column_default FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = ? ORDER BY ordinal_position;";

        let col_rows: Vec<MySqlRow> = sqlx::query(col_query)
            .bind(table)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                CoreError::QueryError(format!("Error al describir columnas en MySQL: {e}"))
            })?;

        if col_rows.is_empty() {
            return Err(CoreError::NotFound(format!(
                "La tabla '{table}' no existe en la base de datos MySQL activa"
            )));
        }

        let mut columns = Vec::with_capacity(col_rows.len());
        for r in col_rows {
            let name: String = r.try_get("column_name").unwrap_or_default();
            let data_type: String = r.try_get("data_type").unwrap_or_default();
            let is_nullable_str: String = r.try_get("is_nullable").unwrap_or_default();
            let is_nullable = is_nullable_str.eq_ignore_ascii_case("YES");
            let col_key: String = r.try_get("column_key").unwrap_or_default();
            let is_primary_key = col_key.eq_ignore_ascii_case("PRI");
            let default_value: Option<String> = r.try_get("column_default").ok();

            columns.push(ColumnSchema {
                name,
                data_type,
                is_nullable,
                is_primary_key,
                default_value,
            });
        }

        // 2. Obtener claves foráneas
        let fk_query = "SELECT constraint_name, column_name, referenced_table_name, referenced_column_name FROM information_schema.key_column_usage WHERE table_schema = DATABASE() AND table_name = ? AND referenced_table_name IS NOT NULL;";

        let fk_rows: Vec<MySqlRow> = sqlx::query(fk_query)
            .bind(table)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                CoreError::QueryError(format!("Error al obtener claves foráneas en MySQL: {e}"))
            })?;

        let mut foreign_keys = Vec::new();
        for r in fk_rows {
            let constraint_name: Option<String> = r.try_get("constraint_name").ok();
            let from_column: String = r.try_get("column_name").unwrap_or_default();
            let to_table: String = r.try_get("referenced_table_name").unwrap_or_default();
            let to_column: String = r.try_get("referenced_column_name").unwrap_or_default();

            foreign_keys.push(ForeignKeySchema {
                constraint_name,
                from_column,
                to_table,
                to_column,
                is_logical: false,
            });
        }

        Ok(TableSchema {
            name: table.to_string(),
            schema_name: None,
            columns,
            foreign_keys,
            estimated_row_count: None,
        })
    }

    async fn close(&self) -> Result<(), CoreError> {
        self.pool.close().await;
        Ok(())
    }
}

/// Convierte dinámicamente un valor de MySqlRow en el tipo canónico `lyra_core::Value`
fn extract_mysql_value(row: &MySqlRow, col_idx: usize, type_name: &str) -> Value {
    if let Ok(raw) = row.try_get_raw(col_idx) {
        if raw.is_null() {
            return Value::Null;
        }
    }

    let upper_type = type_name.to_uppercase();

    match upper_type.as_str() {
        "BOOLEAN" | "BOOL" => {
            if let Ok(b) = row.try_get::<bool, _>(col_idx) {
                Value::Boolean(b)
            } else if let Ok(i) = row.try_get::<i8, _>(col_idx) {
                Value::Boolean(i != 0)
            } else {
                Value::Null
            }
        }
        "TINYINT" => {
            if let Ok(i) = row.try_get::<i8, _>(col_idx) {
                Value::Integer(i as i64)
            } else if let Ok(u) = row.try_get::<u8, _>(col_idx) {
                Value::Integer(u as i64)
            } else if let Ok(b) = row.try_get::<bool, _>(col_idx) {
                Value::Boolean(b)
            } else {
                Value::Null
            }
        }
        "SMALLINT" => {
            if let Ok(i) = row.try_get::<i16, _>(col_idx) {
                Value::Integer(i as i64)
            } else if let Ok(u) = row.try_get::<u16, _>(col_idx) {
                Value::Integer(u as i64)
            } else {
                Value::Null
            }
        }
        "MEDIUMINT" | "INT" | "INTEGER" => {
            if let Ok(i) = row.try_get::<i32, _>(col_idx) {
                Value::Integer(i as i64)
            } else if let Ok(u) = row.try_get::<u32, _>(col_idx) {
                Value::Integer(u as i64)
            } else {
                Value::Null
            }
        }
        "BIGINT" => {
            if let Ok(i) = row.try_get::<i64, _>(col_idx) {
                Value::Integer(i)
            } else if let Ok(u) = row.try_get::<u64, _>(col_idx) {
                Value::Integer(u as i64)
            } else {
                Value::Null
            }
        }
        "FLOAT" => {
            if let Ok(f) = row.try_get::<f32, _>(col_idx) {
                Value::Real(f as f64)
            } else {
                Value::Null
            }
        }
        "DOUBLE" => {
            if let Ok(f) = row.try_get::<f64, _>(col_idx) {
                Value::Real(f)
            } else {
                Value::Null
            }
        }
        "DECIMAL" | "NUMERIC" => {
            if let Ok(f) = row.try_get::<f64, _>(col_idx) {
                Value::Real(f)
            } else if let Ok(s) = row.try_get::<String, _>(col_idx) {
                Value::Text(s)
            } else {
                Value::Null
            }
        }
        "VARCHAR" | "CHAR" | "TEXT" | "TINYTEXT" | "MEDIUMTEXT" | "LONGTEXT" | "ENUM" | "SET" => {
            if let Ok(s) = row.try_get::<String, _>(col_idx) {
                Value::Text(s)
            } else {
                Value::Null
            }
        }
        "JSON" => {
            if let Ok(s) = row.try_get::<String, _>(col_idx) {
                Value::Json(serde_json::from_str(&s).unwrap_or(serde_json::Value::String(s)))
            } else if let Ok(bytes) = row.try_get::<Vec<u8>, _>(col_idx) {
                if let Ok(s) = std::str::from_utf8(&bytes) {
                    Value::Json(
                        serde_json::from_str(s)
                            .unwrap_or_else(|_| serde_json::Value::String(s.to_string())),
                    )
                } else {
                    Value::Blob(bytes)
                }
            } else {
                Value::Null
            }
        }
        "DATETIME" | "TIMESTAMP" | "DATE" | "TIME" => {
            if let Ok(s) = row.try_get::<String, _>(col_idx) {
                Value::DateTime(s)
            } else {
                Value::Null
            }
        }
        "BLOB" | "VARBINARY" | "BINARY" | "TINYBLOB" | "MEDIUMBLOB" | "LONGBLOB" => {
            if let Ok(bytes) = row.try_get::<Vec<u8>, _>(col_idx) {
                Value::Blob(bytes)
            } else {
                Value::Null
            }
        }
        _ => {
            if let Ok(s) = row.try_get::<String, _>(col_idx) {
                Value::Text(s)
            } else if let Ok(i) = row.try_get::<i64, _>(col_idx) {
                Value::Integer(i)
            } else if let Ok(f) = row.try_get::<f64, _>(col_idx) {
                Value::Real(f)
            } else {
                Value::Null
            }
        }
    }
}
