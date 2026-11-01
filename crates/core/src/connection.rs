use crate::error::CoreError;
use crate::result::QueryResult;
use crate::schema::TableSchema;
use async_trait::async_trait;

/// Trait principal que debe implementar todo driver en Lyra (SQLite, PostgreSQL, MongoDB, Redis...).
///
/// Este contrato garantiza que la interfaz de usuario (GTK) y la CLI puedan tratar
/// a cualquier motor de base de datos de manera uniforme.
#[async_trait]
pub trait DatabaseConnection: Send + Sync {
    /// Nombre identificador del motor (ej. "SQLite", "PostgreSQL", "MongoDB").
    fn engine_name(&self) -> &'static str;

    /// Comprueba la salud y disponibilidad de la conexión.
    async fn ping(&self) -> Result<(), CoreError>;

    /// Ejecuta una consulta o comando y devuelve el resultado unificado.
    async fn execute(&self, query: &str) -> Result<QueryResult, CoreError>;

    /// Lista todas las tablas o colecciones existentes.
    async fn list_tables(&self) -> Result<Vec<String>, CoreError>;

    /// Inspecciona el esquema detallado de una tabla o colección.
    async fn describe_table(&self, table: &str) -> Result<TableSchema, CoreError>;

    /// Cierra la conexión de forma limpia.
    async fn close(&self) -> Result<(), CoreError>;
}
