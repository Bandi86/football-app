use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredArtifact {
    pub sha256: String,
    pub storage_key: String,
    pub byte_size: u64,
    pub compression: &'static str,
}

pub struct ObjectStore {
    root: PathBuf,
}

impl ObjectStore {
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().join("raw");
        fs::create_dir_all(&root).context("nyers objektumtár létrehozása")?;
        Ok(Self { root })
    }

    pub fn put_json(&self, payload: &[u8]) -> Result<StoredArtifact> {
        let sha256 = hex::encode(Sha256::digest(payload));
        let relative = format!("{}/{}/{}.json.zst", &sha256[0..2], &sha256[2..4], sha256);
        let path = self.root.join(&relative);

        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let compressed = zstd::encode_all(payload, 3)?;
            let temp = path.with_extension("tmp");
            let mut file = fs::File::create(&temp)?;
            file.write_all(&compressed)?;
            file.sync_all()?;
            fs::rename(temp, &path)?;
        }

        Ok(StoredArtifact {
            sha256,
            storage_key: relative,
            byte_size: payload.len() as u64,
            compression: "zstd",
        })
    }

    pub fn get(&self, artifact: &StoredArtifact) -> Result<Vec<u8>> {
        let path = self.root.join(&artifact.storage_key);
        let compressed = fs::read(path)?;
        Ok(zstd::decode_all(compressed.as_slice())?)
    }
}
