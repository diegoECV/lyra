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

    /// Exporta los datos de la consulta a formato CSV estándar con encabezados
    pub fn to_csv(&self) -> String {
        let mut buffer = String::new();

        // 1. Encabezados
        let headers: Vec<String> = self
            .columns
            .iter()
            .map(|c| escape_csv_field(&c.name))
            .collect();
        buffer.push_str(&headers.join(","));
        buffer.push('\n');

        // 2. Filas
        for row in &self.rows {
            let row_fields: Vec<String> = row
                .values
                .iter()
                .map(|v| escape_csv_field(&v.to_string()))
                .collect();
            buffer.push_str(&row_fields.join(","));
            buffer.push('\n');
        }

        buffer
    }

    /// Exporta los datos a un array JSON de objetos clave-valor
    pub fn to_json_value(&self) -> serde_json::Value {
        let mut json_rows = Vec::with_capacity(self.rows.len());

        for row in &self.rows {
            let mut map = serde_json::Map::new();
            for (col, val) in self.columns.iter().zip(row.values.iter()) {
                let json_val = match val {
                    Value::Null => serde_json::Value::Null,
                    Value::Boolean(b) => serde_json::Value::Bool(*b),
                    Value::Integer(i) => serde_json::Value::Number((*i).into()),
                    Value::Real(r) => serde_json::Number::from_f64(*r)
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::Null),
                    Value::Text(t) => serde_json::Value::String(t.clone()),
                    Value::Blob(b) => {
                        serde_json::Value::String(format!("<blob: {} bytes>", b.len()))
                    }
                    Value::DateTime(dt) => serde_json::Value::String(dt.clone()),
                    Value::Json(j) => j.clone(),
                };
                map.insert(col.name.clone(), json_val);
            }
            json_rows.push(serde_json::Value::Object(map));
        }

        serde_json::Value::Array(json_rows)
    }

    /// Exporta los datos a una cadena formateada JSON
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.to_json_value())
    }
}

fn escape_csv_field(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        let escaped = field.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        field.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_export() {
        let cols = vec![
            ColumnMetadata::new("id", "INTEGER", false),
            ColumnMetadata::new("nombre", "TEXT", false),
            ColumnMetadata::new("nota", "TEXT", true),
        ];
        let rows = vec![
            Row::new(vec![
                Value::Integer(1),
                Value::Text("Diego".to_string()),
                Value::Text("Nota con, coma".to_string()),
            ]),
            Row::new(vec![
                Value::Integer(2),
                Value::Text("Valeria \"VIP\"".to_string()),
                Value::Null,
            ]),
        ];
        let q_res = QueryResult {
            columns: cols,
            rows,
            rows_affected: 0,
            execution_time_ms: 1,
        };

        let csv = q_res.to_csv();
        assert!(csv.starts_with("id,nombre,nota\n"));
        assert!(csv.contains("1,Diego,\"Nota con, coma\"\n"));
        assert!(csv.contains("2,\"Valeria \"\"VIP\"\"\",NULL\n"));
    }

    #[test]
    fn test_json_export() {
        let cols = vec![
            ColumnMetadata::new("id", "INTEGER", false),
            ColumnMetadata::new("activo", "BOOLEAN", false),
        ];
        let rows = vec![Row::new(vec![Value::Integer(10), Value::Boolean(true)])];
        let q_res = QueryResult {
            columns: cols,
            rows,
            rows_affected: 0,
            execution_time_ms: 1,
        };

        let json = q_res.to_json_pretty().expect("json serialize");
        assert!(json.contains("\"id\": 10"));
        assert!(json.contains("\"activo\": true"));
    }
}
