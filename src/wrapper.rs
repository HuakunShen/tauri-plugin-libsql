use futures::lock::Mutex;
use futures::FutureExt;
use indexmap::IndexMap;
use libsql::{params::Params, Builder as LibsqlBuilder, Connection, Database, Value};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use crate::decode;
use crate::error::Error;
use crate::models::{EncryptionConfig, QueryResult};

/// A wrapper around libsql connection
pub struct DbConnection {
    conn: Connection,
    db: Database,
}

impl DbConnection {
    /// Connect to a libsql database.
    ///
    /// - Local only: `sync_url` = None
    /// - Embedded replica (Turso): `sync_url` = Some("libsql://…"), `auth_token` = Some("…")
    /// - Pure remote: `path` starts with "libsql://" or "https://", no `sync_url`
    pub async fn connect(
        path: &str,
        encryption: Option<EncryptionConfig>,
        base_path: PathBuf,
        sync_url: Option<String>,
        auth_token: Option<String>,
    ) -> Result<Self, Error> {
        // Wrap in catch_unwind: libsql's builder calls unwrap() internally and can
        // panic on a malformed URL, which would cause the Tauri IPC to hang forever.
        let path = path.to_string();
        let db = AssertUnwindSafe(async move {
            if let Some(url) = sync_url {
                let full_path = Self::resolve_local_path(&path, &base_path)?;
                Self::open_replica(full_path, url, auth_token.unwrap_or_default(), encryption).await
            } else if path.starts_with("libsql://") || path.starts_with("https://") {
                Self::open_remote(path, auth_token.unwrap_or_default()).await
            } else {
                let full_path = Self::resolve_local_path(&path, &base_path)?;
                Self::open_local(full_path, encryption).await
            }
        })
        .catch_unwind()
        .await
        .map_err(|_| {
            Error::InvalidDbUrl(
                "libsql panicked building the database — check your URL format \
                 (expected libsql://… or https://…)"
                    .into(),
            )
        })??;

        let conn = db.connect()?;
        Ok(Self { conn, db })
    }

    // ── connection mode helpers ──────────────────────────────────────────────

    fn resolve_local_path(path: &str, base_path: &Path) -> Result<PathBuf, Error> {
        let db_path = path.strip_prefix("sqlite:").unwrap_or(path);

        if db_path == ":memory:" {
            return Ok(PathBuf::from(":memory:"));
        }

        if PathBuf::from(db_path).is_absolute() {
            return Ok(PathBuf::from(db_path));
        }

        // Normalise away `..` so a path can't escape base_path
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

        if !normalised.starts_with(base_path) {
            return Err(Error::InvalidDbUrl(format!(
                "path '{}' escapes the base directory",
                db_path
            )));
        }

        Ok(normalised)
    }

    async fn open_local(
        full_path: PathBuf,
        encryption: Option<EncryptionConfig>,
    ) -> Result<Database, Error> {
        #[allow(unused_mut)]
        let mut builder = LibsqlBuilder::new_local(&full_path.to_string_lossy().to_string());

        #[cfg(feature = "encryption")]
        if let Some(config) = encryption {
            builder = builder.encryption_config(config.into());
        }
        #[cfg(not(feature = "encryption"))]
        if encryption.is_some() {
            return Err(Error::InvalidDbUrl(
                "encryption feature is not enabled — rebuild with the `encryption` feature".into(),
            ));
        }

        Ok(builder.build().await?)
    }

    #[cfg(feature = "replication")]
    async fn open_replica(
        full_path: PathBuf,
        sync_url: String,
        auth_token: String,
        encryption: Option<EncryptionConfig>,
    ) -> Result<Database, Error> {
        #[allow(unused_mut)]
        let mut builder = LibsqlBuilder::new_remote_replica(
            full_path.to_string_lossy().to_string(),
            sync_url,
            auth_token,
        );

        #[cfg(feature = "encryption")]
        if let Some(config) = encryption {
            builder = builder.encryption_config(config.into());
        }

        let db = builder.build().await?;
        // Initial sync so the local replica is up-to-date on connect
        db.sync().await?;
        Ok(db)
    }

    #[cfg(not(feature = "replication"))]
    async fn open_replica(
        _full_path: PathBuf,
        _sync_url: String,
        _auth_token: String,
        _encryption: Option<EncryptionConfig>,
    ) -> Result<Database, Error> {
        Err(Error::InvalidDbUrl(
            "embedded replica requires the `replication` feature — add features = [\"replication\"] to your Cargo.toml".into(),
        ))
    }

    #[cfg(feature = "remote")]
    async fn open_remote(url: String, auth_token: String) -> Result<Database, Error> {
        Ok(LibsqlBuilder::new_remote(url, auth_token).build().await?)
    }

    #[cfg(not(feature = "remote"))]
    async fn open_remote(_url: String, _auth_token: String) -> Result<Database, Error> {
        Err(Error::InvalidDbUrl(
            "remote connections require the `remote` feature — add features = [\"remote\"] to your Cargo.toml".into(),
        ))
    }

    // ── public API ───────────────────────────────────────────────────────────

    /// Sync an embedded replica with its remote database.
    /// No-op (returns Ok) for local-only databases when replication is disabled.
    pub async fn sync(&self) -> Result<(), Error> {
        Self::do_sync(&self.db).await
    }

    #[cfg(feature = "replication")]
    async fn do_sync(db: &Database) -> Result<(), Error> {
        db.sync().await?;
        Ok(())
    }

    #[cfg(not(feature = "replication"))]
    async fn do_sync(_db: &Database) -> Result<(), Error> {
        Err(Error::OperationNotSupported(
            "sync requires the `replication` feature".into(),
        ))
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

    /// Execute multiple SQL statements atomically inside a transaction.
    /// Statements must not contain bound parameters — use for DDL and bulk DML only.
    pub async fn batch(&self, queries: Vec<String>) -> Result<(), Error> {
        self.conn.execute("BEGIN", Params::None).await?;
        for query in &queries {
            if let Err(e) = self.conn.execute(query.as_str(), Params::None).await {
                let _ = self.conn.execute("ROLLBACK", Params::None).await;
                return Err(Error::Libsql(e));
            }
        }
        if let Err(e) = self.conn.execute("COMMIT", Params::None).await {
            let _ = self.conn.execute("ROLLBACK", Params::None).await;
            return Err(Error::Libsql(e));
        }
        Ok(())
    }

    pub async fn close(&self) {
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
            if arr.iter().all(|v| v.is_number()) {
                let bytes: Vec<u8> = arr
                    .iter()
                    .filter_map(|v| v.as_u64().map(|n| n as u8))
                    .collect();
                Value::Blob(bytes)
            } else {
                Value::Text(v.to_string())
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
