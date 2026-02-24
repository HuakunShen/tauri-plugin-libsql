use serde::de::DeserializeOwned;
use std::path::PathBuf;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::*;

/// Plugin configuration
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Base path for relative database paths. Defaults to current working directory.
    pub base_path: Option<PathBuf>,
    /// Default encryption configuration for all databases.
    /// Can be overridden per-database when loading.
    pub encryption: Option<EncryptionConfig>,
}

pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    _api: PluginApi<R, C>,
    config: Config,
) -> crate::Result<Libsql> {
    Ok(Libsql(config))
}

/// Access to the libsql APIs.
pub struct Libsql(Config);

impl Libsql {
    pub fn ping(&self, payload: PingRequest) -> crate::Result<PingResponse> {
        Ok(PingResponse {
            value: payload.value,
        })
    }

    /// Get the configured base path for databases
    pub fn base_path(&self) -> PathBuf {
        self.0
            .base_path
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    /// Get the default encryption config
    pub fn encryption(&self) -> Option<&EncryptionConfig> {
        self.0.encryption.as_ref()
    }
}
