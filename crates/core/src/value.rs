use serde::{Deserialize, Serialize};
use std::fmt;

/// Representación canónica y polimórfica de cualquier valor devuelto por un motor (SQL o NoSQL).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
    DateTime(String),
    Json(serde_json::Value),
}

impl Value {
    /// Nombre del tipo de dato en texto legible.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "NULL",
            Value::Boolean(_) => "BOOLEAN",
            Value::Integer(_) => "INTEGER",
            Value::Real(_) => "REAL",
            Value::Text(_) => "TEXT",
            Value::Blob(_) => "BLOB",
            Value::DateTime(_) => "DATETIME",
            Value::Json(_) => "JSON",
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "NULL"),
            Value::Boolean(b) => write!(f, "{}", b),
            Value::Integer(i) => write!(f, "{}", i),
            Value::Real(r) => write!(f, "{}", r),
            Value::Text(s) => write!(f, "{}", s),
            Value::Blob(bytes) => write!(f, "<BLOB {} bytes>", bytes.len()),
            Value::DateTime(dt) => write!(f, "{}", dt),
            Value::Json(j) => write!(f, "{}", j),
        }
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Boolean(v)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Integer(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Value::Integer(v as i64)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Real(v)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Text(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Text(v.to_string())
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Value::Blob(v)
    }
}

impl From<serde_json::Value> for Value {
    fn from(v: serde_json::Value) -> Self {
        Value::Json(v)
    }
}
