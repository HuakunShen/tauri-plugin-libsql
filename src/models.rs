use serde::{Deserialize, Serialize};

/// Cipher types for encryption
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Cipher {
    #[serde(rename = "aes256cbc", alias = "aes256-cbc")]
    Aes256Cbc,
}

#[cfg(feature = "encryption")]
impl From<Cipher> for libsql::Cipher {
    fn from(cipher: Cipher) -> Self {
        match cipher {
            Cipher::Aes256Cbc => libsql::Cipher::Aes256Cbc,
        }
    }
}

/// Encryption configuration for database
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptionConfig {
    pub cipher: Cipher,
    pub key: Vec<u8>,
}

#[cfg(feature = "encryption")]
impl From<EncryptionConfig> for libsql::EncryptionConfig {
    fn from(config: EncryptionConfig) -> Self {
        libsql::EncryptionConfig::new(config.cipher.into(), bytes::Bytes::from(config.key))
    }
}

/// Options for loading a database
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadOptions {
    /// Database path (e.g., "sqlite:test.db" or just "test.db")
    pub path: String,
    /// Optional encryption configuration
    pub encryption: Option<EncryptionConfig>,
}

/// Result of an execute operation
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    /// Number of rows affected
    pub rows_affected: u64,
    /// Last inserted row ID
    pub last_insert_id: i64,
}

// Keep ping for backwards compatibility
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PingRequest {
    pub value: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PingResponse {
    pub value: Option<String>,
}
