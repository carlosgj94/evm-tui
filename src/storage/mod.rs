use color_eyre::Result;
use fjall::{Config, Keyspace, PartitionCreateOptions};
use std::{
    fs,
    path::{Path, PathBuf},
};

mod repositories;

pub use repositories::{
    FavoriteRecord, FavoritesRepository, SecretKey, SecretsRepository, SettingsRepository,
};

pub struct Storage {
    #[allow(dead_code)]
    keyspace: Keyspace,
    favorites_addresses: FavoritesRepository,
    favorites_transactions: FavoritesRepository,
    settings: SettingsRepository,
    secrets: SecretsRepository,
}

impl Storage {
    pub fn open_default() -> Result<Self> {
        let root = default_data_dir()?;
        Self::open(root)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let root = path.as_ref();
        fs::create_dir_all(root)?;

        let keyspace = Config::new(root).open()?;
        let favorites_addresses =
            keyspace.open_partition("favorites_addresses", PartitionCreateOptions::default())?;
        let favorites_transactions =
            keyspace.open_partition("favorites_transactions", PartitionCreateOptions::default())?;
        let settings = keyspace.open_partition("settings", PartitionCreateOptions::default())?;
        let secrets = keyspace.open_partition("secrets", PartitionCreateOptions::default())?;

        Ok(Self {
            favorites_addresses: FavoritesRepository::new(favorites_addresses),
            favorites_transactions: FavoritesRepository::new(favorites_transactions),
            settings: SettingsRepository::new(settings),
            secrets: SecretsRepository::new(secrets),
            keyspace,
        })
    }

    pub fn favorites_addresses(&self) -> &FavoritesRepository {
        &self.favorites_addresses
    }

    pub fn favorites_transactions(&self) -> &FavoritesRepository {
        &self.favorites_transactions
    }

    pub fn settings(&self) -> &SettingsRepository {
        &self.settings
    }

    pub fn secrets(&self) -> &SecretsRepository {
        &self.secrets
    }
}

fn default_data_dir() -> Result<PathBuf> {
    let explicit = std::env::var("EVM_TUI_DATA_DIR").map(PathBuf::from);
    let path = match explicit {
        Ok(path) => path,
        Err(_) => {
            let mut root = dirs::data_local_dir()
                .unwrap_or(std::env::current_dir()?)
                .join("evm-tui");
            if cfg!(debug_assertions) {
                root = root.join("dev");
            }
            root
        }
    };
    Ok(path)
}
