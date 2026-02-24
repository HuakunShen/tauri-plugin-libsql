use futures::lock::Mutex;
use indexmap::IndexMap;
use libsql::{params::Params, Builder as LibsqlBuilder, Connection, Database, Value};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::{Component, PathBuf};
use std::sync::Arc;

use crate::decode;
use crate::error::Error;
use crate::models::{EncryptionConfig, QueryResult};

/// A wrapper around libsql connection
pub struct DbConnection {
    conn: Connection,
    #[allow(dead_code)]
    db: Database,
}

impl DbConnection {
    /// Connect to a libsql database
    pub async fn connect(
        path: &str,
        encryption: Option<EncryptionConfig>,
        base_path: PathBuf,
    ) -> Result<Self, Error> {
        // Parse path - handle both "sqlite:path" and plain "path" formats
        let db_path = path.strip_prefix("sqlite:").unwrap_or(path);

        // Build full path - use base_path for relative paths
        let full_path = if db_path == ":memory:" {
            PathBuf::from(":memory:")
        } else if PathBuf::from(db_path).is_absolute() {
            // Use absolute paths as-is
            PathBuf::from(db_path)
        } else {
            // For relative paths, join with base_path then normalise away any
            // `..` components so a path like "../../etc/passwd" can't escape.
            let joined = base_path.join(db_path);
            let normalised = joined.components().fold(PathBuf::new(), |mut acc, c| {
                match c {
                    Component::ParentDir => {
                        acc.pop();
                    }
                    Component::CurDir => {}
                    _ => acc.push(c),
                }
                acc
            });
            if !normalised.starts_with(&base_path) {
                return Err(crate::Error::InvalidDbUrl(format!(
                    "path '{}' escapes the base directory",
                    db_path
                )));
            }
            normalised
        };

        // Use the new Builder pattern
        let mut builder = LibsqlBuilder::new_local(&full_path.to_string_lossy().to_string());

        // Apply encryption config if provided
        if let Some(config) = encryption {
            builder = builder.encryption_config(config.into());
        }

        let db = builder.build().await?;
        let conn = db.connect()?;

        Ok(Self { conn, db })
    }

    /// Execute a query that doesn't return rows
    pub async fn execute(&self, query: &str, values: Vec<JsonValue>) -> Result<QueryResult, Error> {
        let params = json_to_params(values);
        let rows_affected = self.conn.execute(query, params).await?;

        Ok(QueryResult {
            rows_affected,
            last_insert_id: self.conn.last_insert_rowid(),
        })
    }

    /// Execute a query that returns rows
    pub async fn select(
        &self,
        query: &str,
        values: Vec<JsonValue>,
    ) -> Result<Vec<IndexMap<String, JsonValue>>, Error> {
        let params = json_to_params(values);
        let mut rows = self.conn.query(query, params).await?;

        let mut results = Vec::new();

        while let Some(row) = rows.next().await? {
            let mut map = IndexMap::new();
            let column_count = row.column_count();

            for i in 0..column_count {
                if let Some(column_name) = row.column_name(i) {
                    let value = decode::to_json(&row, i)?;
                    map.insert(column_name.to_string(), value);
                }
            }

            results.push(map);
        }

        Ok(results)
    }

    /// Close the connection (database is dropped when struct is dropped)
    pub async fn close(&self) {
        // libsql doesn't have explicit close - connection closes on drop
        // We can reset the connection state if needed
        self.conn.reset().await;
    }
}

/// Convert JSON values to libsql params
fn json_to_params(values: Vec<JsonValue>) -> Params {
    if values.is_empty() {
        return Params::None;
    }

    let params: Vec<Value> = values.into_iter().map(json_to_libsql_value).collect();

    Params::Positional(params)
}

/// Convert a JSON value to a libsql value
fn json_to_libsql_value(v: JsonValue) -> Value {
    match v {
        JsonValue::Null => Value::Null,
        JsonValue::Bool(b) => Value::Integer(if b { 1 } else { 0 }),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                Value::Real(f)
            } else {
                Value::Null
            }
        }
        JsonValue::String(s) => Value::Text(s),
        JsonValue::Array(ref arr) => {
            // Convert array to blob if all numbers, otherwise serialize to JSON string
            if arr.iter().all(|v| v.is_number()) {
                let bytes: Vec<u8> = arr
                    .iter()
                    .filter_map(|v| v.as_u64().map(|n| n as u8))
                    .collect();
                Value::Blob(bytes)
            } else {
                let json_str = v.to_string();
                Value::Text(json_str)
            }
        }
        JsonValue::Object(_) => Value::Text(v.to_string()),
    }
}

/// Database instances holder
pub struct DbInstances(pub Arc<Mutex<HashMap<String, Arc<DbConnection>>>>);

impl Default for DbInstances {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(HashMap::new())))
    }
}
