//! Output formatters for mws.

use std::io::Write;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Table,
    Yaml,
    Tsv,
}

impl Format {
    /// Pick a default based on whether stdout is a TTY.
    pub fn auto(is_tty: bool) -> Self {
        if is_tty { Self::Table } else { Self::Json }
    }

    pub fn parse(s: &str) -> Result<Self, FormatError> {
        match s {
            "json" => Ok(Self::Json),
            "table" => Ok(Self::Table),
            "yaml" => Ok(Self::Yaml),
            "tsv" => Ok(Self::Tsv),
            other => Err(FormatError::Unknown(other.to_string())),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("unknown output format: {0}")]
    Unknown(String),
    #[error("serialization failed: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Write a serializable value to `out` using the chosen format.
///
/// For M0, only `Json` and `Table` are fully implemented; `Yaml`/`Tsv` fall back to JSON.
pub fn write<T: Serialize, W: Write>(format: Format, value: &T, out: &mut W) -> Result<(), FormatError> {
    let v = serde_json::to_value(value)?;
    match format {
        Format::Json => {
            serde_json::to_writer_pretty(&mut *out, &v)?;
            writeln!(out)?;
        }
        Format::Table => write_table(&v, out)?,
        Format::Yaml | Format::Tsv => {
            serde_json::to_writer_pretty(&mut *out, &v)?;
            writeln!(out)?;
        }
    }
    Ok(())
}

fn write_table<W: Write>(v: &serde_json::Value, out: &mut W) -> Result<(), FormatError> {
    use comfy_table::Table;
    let mut table = Table::new();
    match v {
        serde_json::Value::Object(map) => {
            table.set_header(["field", "value"]);
            for (k, val) in map {
                table.add_row([k.as_str(), &val_to_cell(val)]);
            }
        }
        serde_json::Value::Array(arr) => {
            if let Some(first) = arr.iter().find_map(|e| e.as_object()) {
                let headers: Vec<String> = first.keys().cloned().collect();
                table.set_header(headers.iter().map(|s| s.as_str()));
                for row in arr {
                    if let Some(obj) = row.as_object() {
                        let cells: Vec<String> = headers.iter().map(|h| val_to_cell(obj.get(h).unwrap_or(&serde_json::Value::Null))).collect();
                        table.add_row(cells);
                    }
                }
            } else {
                for row in arr {
                    table.add_row([val_to_cell(row)]);
                }
            }
        }
        _ => {
            table.add_row([val_to_cell(v)]);
        }
    }
    writeln!(out, "{table}")?;
    Ok(())
}

fn val_to_cell(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn auto_picks_json_for_pipes() {
        assert_eq!(Format::auto(false), Format::Json);
        assert_eq!(Format::auto(true), Format::Table);
    }

    #[test]
    fn parse_known_formats() {
        assert_eq!(Format::parse("json").unwrap(), Format::Json);
        assert_eq!(Format::parse("table").unwrap(), Format::Table);
        assert!(Format::parse("xml").is_err());
    }

    #[test]
    fn json_round_trip_object() {
        let mut buf = Vec::new();
        write(Format::Json, &json!({"a": 1, "b": "x"}), &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("\"a\""));
        assert!(s.contains("\"b\""));
    }

    #[test]
    fn table_renders_object_as_field_value() {
        let mut buf = Vec::new();
        write(Format::Table, &json!({"displayName": "Alice"}), &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("displayName"));
        assert!(s.contains("Alice"));
    }
}
