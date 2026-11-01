use async_trait::async_trait;
use lyra_core::{
    ColumnMetadata, ColumnSchema, CoreError, DatabaseConnection, QueryResult, Row as LyraRow,
    TableSchema, Value,
};
use redis::aio::MultiplexedConnection;
use redis::{AsyncCommands, Client};
use std::time::Instant;

pub struct RedisConnection {
    client: Client,
    conn: tokio::sync::Mutex<MultiplexedConnection>,
    url: String,
}

impl RedisConnection {
    /// Conecta a un servidor Redis o Valkey mediante una URL estándar.
    ///
    /// Ejemplo: `redis://127.0.0.1:6379` o `redis://:password@127.0.0.1:6379/0`
    pub async fn connect(url: &str) -> Result<Self, CoreError> {
        let client = Client::open(url)
            .map_err(|e| CoreError::ConnectionFailed(format!("URL de Redis inválida: {e}")))?;

        let conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| CoreError::ConnectionFailed(format!("Fallo conectando a Redis: {e}")))?;

        Ok(Self {
            client,
            conn: tokio::sync::Mutex::new(conn),
            url: url.to_string(),
        })
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}

#[async_trait]
impl DatabaseConnection for RedisConnection {
    fn engine_name(&self) -> &'static str {
        "Redis"
    }

    async fn ping(&self) -> Result<(), CoreError> {
        let mut conn = self.conn.lock().await;
        let response: String = redis::cmd("PING")
            .query_async(&mut *conn)
            .await
            .map_err(|e| CoreError::ConnectionFailed(format!("Fallo de ping en Redis: {e}")))?;

        if response == "PONG" {
            Ok(())
        } else {
            Err(CoreError::ConnectionFailed(format!(
                "Respuesta inesperada de PING: {response}"
            )))
        }
    }

    async fn execute(&self, query: &str) -> Result<QueryResult, CoreError> {
        let start = Instant::now();
        let parts: Vec<&str> = query.split_whitespace().collect();

        if parts.is_empty() {
            return Ok(QueryResult::empty(0));
        }

        let mut cmd = redis::cmd(parts[0]);
        for arg in &parts[1..] {
            cmd.arg(*arg);
        }

        let mut conn = self.conn.lock().await;
        let val: redis::Value = cmd
            .query_async(&mut *conn)
            .await
            .map_err(|e| CoreError::QueryError(format!("Error en comando Redis: {e}")))?;

        let execution_time_ms = start.elapsed().as_millis();
        Ok(redis_value_to_query_result(val, execution_time_ms))
    }

    async fn list_tables(&self) -> Result<Vec<String>, CoreError> {
        let mut conn = self.conn.lock().await;

        // Usar SCAN para listar de forma segura hasta 100 claves sin bloquear el servidor
        let keys: Vec<String> = redis::cmd("SCAN")
            .arg("0")
            .arg("COUNT")
            .arg("100")
            .query_async(&mut *conn)
            .await
            .map(|(_next_cursor, keys): (String, Vec<String>)| keys)
            .unwrap_or_else(|_| Vec::new());

        Ok(keys)
    }

    async fn describe_table(&self, key: &str) -> Result<TableSchema, CoreError> {
        let mut conn = self.conn.lock().await;

        let key_type: String = conn.key_type(key).await.map_err(|e| {
            CoreError::QueryError(format!("Error obteniendo tipo de clave {key}: {e}"))
        })?;

        if key_type == "none" {
            return Err(CoreError::NotFound(key.to_string()));
        }

        let ttl: i64 = conn.ttl(key).await.unwrap_or(-1);

        let ttl_desc = match ttl {
            -1 => "Sin expiración (persistente)".to_string(),
            -2 => "Expirada".to_string(),
            secs => format!("{secs} segundos"),
        };

        let columns = vec![
            ColumnSchema {
                name: "Clave".to_string(),
                data_type: "String".to_string(),
                is_nullable: false,
                is_primary_key: true,
                default_value: None,
            },
            ColumnSchema {
                name: "Tipo".to_string(),
                data_type: key_type,
                is_nullable: false,
                is_primary_key: false,
                default_value: None,
            },
            ColumnSchema {
                name: "TTL".to_string(),
                data_type: "Integer (segundos)".to_string(),
                is_nullable: false,
                is_primary_key: false,
                default_value: Some(ttl_desc),
            },
        ];

        Ok(TableSchema {
            name: key.to_string(),
            schema_name: Some("db0".to_string()),
            columns,
            foreign_keys: Vec::new(),
            estimated_row_count: Some(1),
        })
    }

    async fn close(&self) -> Result<(), CoreError> {
        Ok(())
    }
}

/// Convierte el resultado polimórfico de Redis en un QueryResult estándar.
fn redis_value_to_query_result(val: redis::Value, execution_time_ms: u128) -> QueryResult {
    match val {
        redis::Value::Nil => QueryResult {
            columns: vec![ColumnMetadata::new("Resultado", "NIL", true)],
            rows: vec![LyraRow::new(vec![Value::Null])],
            rows_affected: 0,
            execution_time_ms,
        },
        redis::Value::Int(i) => QueryResult {
            columns: vec![ColumnMetadata::new("Resultado", "INTEGER", false)],
            rows: vec![LyraRow::new(vec![Value::Integer(i)])],
            rows_affected: i as u64,
            execution_time_ms,
        },
        redis::Value::SimpleString(s) => QueryResult {
            columns: vec![ColumnMetadata::new("Resultado", "STRING", false)],
            rows: vec![LyraRow::new(vec![Value::Text(s)])],
            rows_affected: 0,
            execution_time_ms,
        },
        redis::Value::BulkString(bytes) => {
            let text = String::from_utf8_lossy(&bytes).to_string();
            QueryResult {
                columns: vec![ColumnMetadata::new("Resultado", "STRING", false)],
                rows: vec![LyraRow::new(vec![Value::Text(text)])],
                rows_affected: 0,
                execution_time_ms,
            }
        }
        redis::Value::Array(items) => {
            let columns = vec![
                ColumnMetadata::new("Índice", "INTEGER", false),
                ColumnMetadata::new("Valor", "STRING", true),
            ];

            let rows = items
                .into_iter()
                .enumerate()
                .map(|(idx, item)| {
                    let val_str = match item {
                        redis::Value::SimpleString(s) => s,
                        redis::Value::BulkString(bytes) => {
                            String::from_utf8_lossy(&bytes).to_string()
                        }
                        redis::Value::Int(i) => i.to_string(),
                        redis::Value::Nil => "NIL".to_string(),
                        other => format!("{other:?}"),
                    };
                    LyraRow::new(vec![Value::Integer(idx as i64), Value::Text(val_str)])
                })
                .collect();

            QueryResult {
                columns,
                rows,
                rows_affected: 0,
                execution_time_ms,
            }
        }
        redis::Value::Okay => QueryResult::with_affected(1, execution_time_ms),
        other => QueryResult {
            columns: vec![ColumnMetadata::new("Resultado", "RAW", true)],
            rows: vec![LyraRow::new(vec![Value::Text(format!("{other:?}"))])],
            rows_affected: 0,
            execution_time_ms,
        },
    }
}
