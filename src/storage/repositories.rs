use color_eyre::{Result, eyre::WrapErr};
use fjall::PartitionHandle;
use serde::{Deserialize, Serialize};
use serde_json;

#[derive(Clone)]
pub struct FavoritesRepository {
    handle: PartitionHandle,
}

impl FavoritesRepository {
    pub(crate) fn new(handle: PartitionHandle) -> Self {
        Self { handle }
    }

    pub fn list(&self) -> Result<Vec<FavoriteRecord>> {
        let mut items = Vec::new();
        for entry in self.handle.iter() {
            let (key, value) = entry?;
            let mut record: FavoriteRecord = serde_json::from_slice(value.as_ref())
                .wrap_err("failed to deserialize favorite record")?;
            record.identifier =
                String::from_utf8(key.to_vec()).wrap_err("favorite key is not valid UTF-8")?;
            items.push(record);
        }
        Ok(items)
    }

    pub fn upsert(&self, record: &FavoriteRecord) -> Result<()> {
        let stored = serde_json::to_vec(record).wrap_err("failed to serialize favorite record")?;
        self.handle
            .insert(record.identifier.as_bytes(), stored)
            .wrap_err("failed to insert favorite")
    }

    pub fn remove(&self, identifier: &str) -> Result<()> {
        self.handle
            .remove(identifier.as_bytes())
            .wrap_err("failed to remove favorite")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FavoriteRecord {
    pub label: Option<String>,
    pub identifier: String,
    pub chain: String,
}

#[derive(Clone)]
pub struct SettingsRepository {
    handle: PartitionHandle,
}

impl SettingsRepository {
    pub(crate) fn new(handle: PartitionHandle) -> Self {
        Self { handle }
    }

    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self
            .handle
            .get(key.as_bytes())
            .wrap_err("failed to read setting")?
            .map(|v| v.to_vec()))
    }

    pub fn put(&self, key: &str, value: &[u8]) -> Result<()> {
        self.handle
            .insert(key.as_bytes(), value)
            .wrap_err("failed to write setting")
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SecretKey {
    EtherscanApiKey,
    AnvilRpcUrl,
}

impl SecretKey {
    fn storage_key(self) -> &'static str {
        match self {
            SecretKey::EtherscanApiKey => "v1::secret::etherscan_api_key",
            SecretKey::AnvilRpcUrl => "v1::secret::anvil_rpc_url",
        }
    }

    pub fn env_var(self) -> &'static str {
        match self {
            SecretKey::EtherscanApiKey => "ETHERSCAN_API_KEY",
            SecretKey::AnvilRpcUrl => "ANVIL_RPC_URL",
        }
    }
}

#[derive(Clone)]
pub struct SecretsRepository {
    handle: PartitionHandle,
}

impl SecretsRepository {
    pub(crate) fn new(handle: PartitionHandle) -> Self {
        Self { handle }
    }

    pub fn get(&self, key: SecretKey) -> Result<Option<String>> {
        Ok(self
            .handle
            .get(key.storage_key().as_bytes())
            .wrap_err("failed to read secret")?
            .map(|bytes| {
                String::from_utf8(bytes.to_vec()).wrap_err("secret value is not valid UTF-8")
            })
            .transpose()?)
    }

    pub fn set(&self, key: SecretKey, value: &str) -> Result<()> {
        self.handle
            .insert(key.storage_key().as_bytes(), value.as_bytes())
            .wrap_err("failed to write secret")
    }

    pub fn remove(&self, key: SecretKey) -> Result<()> {
        self.handle
            .remove(key.storage_key().as_bytes())
            .wrap_err("failed to remove secret")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fjall::Config;
    use tempfile::tempdir;

    #[test]
    fn secrets_roundtrip() -> Result<()> {
        let temp = tempdir().unwrap();
        let keyspace = Config::new(temp.path()).open()?;
        let handle = keyspace.open_partition("secrets_test", Default::default())?;
        let secrets = SecretsRepository::new(handle);

        assert!(secrets.get(SecretKey::EtherscanApiKey)?.is_none());
        secrets.set(SecretKey::EtherscanApiKey, "secret-value")?;
        assert_eq!(
            secrets.get(SecretKey::EtherscanApiKey)?,
            Some("secret-value".to_string())
        );
        secrets.remove(SecretKey::EtherscanApiKey)?;
        assert!(secrets.get(SecretKey::EtherscanApiKey)?.is_none());

        Ok(())
    }
}
