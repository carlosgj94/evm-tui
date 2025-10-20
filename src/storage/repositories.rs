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
