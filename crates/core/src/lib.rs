pub mod connection;
pub mod error;
pub mod result;
pub mod schema;
pub mod value;

// Re-exports principales para comodidad de los consumidores (drivers, cli, app-gtk)
pub use connection::DatabaseConnection;
pub use error::CoreError;
pub use result::{ColumnMetadata, QueryResult, Row};
pub use schema::{ColumnSchema, ForeignKeySchema, TableSchema};
pub use value::Value;
