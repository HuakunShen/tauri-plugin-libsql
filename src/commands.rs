use indexmap::IndexMap;
use serde_json::Value as JsonValue;
use tauri::{command, AppHandle, Manager, Runtime, State};

use crate::models::{LoadOptions, PingRequest, PingResponse, QueryResult};
use crate::wrapper::DbInstances;
use crate::Error;

#[cfg(desktop)]
use crate::desktop::Libsql;
#[cfg(mobile)]
use crate::mobile::Libsql;

/// Load a database connection
#[command]
pub(crate) async fn load<R: Runtime>(
    app: AppHandle<R>,
    db_instances: State<'_, DbInstances>,
    options: LoadOptions,
) -> Result<String, Error> {
    let path = options.path.clone();

    let libsql = app.state::<Libsql>().inner();
    let base_path = libsql.base_path();

    // Use provided encryption, or fall back to plugin default
    let encryption = options.encryption.or_else(|| libsql.encryption().cloned());

    let conn = crate::wrapper::DbConnection::connect(&path, encryption, base_path).await?;

    db_instances
        .0
        .lock()
        .await
        .insert(path.clone(), std::sync::Arc::new(conn));

    Ok(path)
}

/// Execute a query that doesn't return rows
#[command]
pub(crate) async fn execute(
    db_instances: State<'_, DbInstances>,
    db: String,
    query: String,
    values: Vec<JsonValue>,
) -> Result<QueryResult, Error> {
    // Clone the Arc while holding the lock, then release the lock before
    // awaiting the query so other operations aren't blocked.
    let conn = {
        let instances = db_instances.0.lock().await;
        instances
            .get(&db)
            .ok_or_else(|| Error::DatabaseNotLoaded(db.clone()))?
            .clone()
    };
    conn.execute(&query, values).await
}

/// Execute a query that returns rows
#[command]
pub(crate) async fn select(
    db_instances: State<'_, DbInstances>,
    db: String,
    query: String,
    values: Vec<JsonValue>,
) -> Result<Vec<IndexMap<String, JsonValue>>, Error> {
    // Clone the Arc while holding the lock, then release the lock before
    // awaiting the query so other operations aren't blocked.
    let conn = {
        let instances = db_instances.0.lock().await;
        instances
            .get(&db)
            .ok_or_else(|| Error::DatabaseNotLoaded(db.clone()))?
            .clone()
    };
    conn.select(&query, values).await
}

/// Close a database connection
#[command]
pub(crate) async fn close(
    db_instances: State<'_, DbInstances>,
    db: Option<String>,
) -> Result<bool, Error> {
    let mut instances = db_instances.0.lock().await;

    if let Some(db) = db {
        if let Some(conn) = instances.remove(&db) {
            conn.close().await;
        }
    } else {
        // Close all connections
        for (_, conn) in instances.drain() {
            conn.close().await;
        }
    }

    Ok(true)
}

/// Ping command (for backwards compatibility)
#[command]
pub(crate) async fn ping<R: Runtime>(
    app: AppHandle<R>,
    payload: PingRequest,
) -> Result<PingResponse, Error> {
    let libsql = app.state::<Libsql>().inner();
    libsql.ping(payload)
}

/// Get plugin config info
#[command]
pub(crate) async fn get_config<R: Runtime>(app: AppHandle<R>) -> Result<ConfigInfo, Error> {
    let libsql = app.state::<Libsql>().inner();
    Ok(ConfigInfo {
        encrypted: libsql.encryption().is_some(),
    })
}

/// Config info returned to frontend
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ConfigInfo {
    pub encrypted: bool,
}
