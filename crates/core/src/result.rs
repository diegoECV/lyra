use crate::value::Value;
use serde::{Deserialize, Serialize};

/// Metadatos sobre una columna resultante de una consulta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnMetadata {
    pub name: String,
    pub type_name: String,
    pub nullable: bool,
}

impl ColumnMetadata {
    pub fn new(name: impl Into<String>, type_name: impl Into<String>, nullable: bool) -> Self {
        Self {
            name: name.into(),
            type_name: type_name.into(),
            nullable,
        }
    }
}

/// Una fila individual de resultados.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub values: Vec<Value>,
}

impl Row {
    pub fn new(values: Vec<Value>) -> Self {
        Self { values }
    }

    pub fn get(&self, index: usize) -> Option<&Value> {
        self.values.get(index)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// Resultado completo de la ejecución de una consulta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    /// Columnas del resultado (vacío si fue una operación DDL/DML como INSERT/UPDATE).
    pub columns: Vec<ColumnMetadata>,
    /// Filas devueltas.
    pub rows: Vec<Row>,
    /// Número de filas afectadas (para INSERT, UPDATE, DELETE).
    pub rows_affected: u64,
    /// Tiempo total de ejecución en milisegundos.
    pub execution_time_ms: u128,
}

impl QueryResult {
    pub fn empty(execution_time_ms: u128) -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            rows_affected: 0,
            execution_time_ms,
        }
    }

    pub fn with_affected(rows_affected: u64, execution_time_ms: u128) -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            rows_affected,
            execution_time_ms,
        }
    }
}
