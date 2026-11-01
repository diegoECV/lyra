use serde::{Deserialize, Serialize};

/// Información sobre una columna de una tabla o colección.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub is_primary_key: bool,
    pub default_value: Option<String>,
}

/// Clave foránea o relación lógica entre dos entidades.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForeignKeySchema {
    pub constraint_name: Option<String>,
    pub from_column: String,
    pub to_table: String,
    pub to_column: String,
    pub is_logical: bool, // true si es una relación mixta SQL-NoSQL o definida por el usuario
}

/// Esquema completo de una tabla o colección.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableSchema {
    pub name: String,
    pub schema_name: Option<String>,
    pub columns: Vec<ColumnSchema>,
    pub foreign_keys: Vec<ForeignKeySchema>,
    pub estimated_row_count: Option<u64>,
}
