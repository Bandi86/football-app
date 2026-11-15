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

    /// Asset-blob (pl. logó) tartalom-címzett mentése, tömörítés nélkül:
    /// `assets/<aa>/<bb>/<sha256>.<ext>` a nyers objektumtár gyökere alatt.
    pub fn put_blob(&self, payload: &[u8], ext: &str) -> Result<StoredArtifact> {
        let sha256 = hex::encode(Sha256::digest(payload));
        let relative = format!(
            "assets/{}/{}/{}.{}",
            &sha256[0..2],
            &sha256[2..4],
            sha256,
            ext
        );
        let path = self.root.join(&relative);
        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let temp = path.with_extension("tmp");
            let mut file = fs::File::create(&temp)?;
            file.write_all(payload)?;
            file.sync_all()?;
            fs::rename(temp, &path)?;
        }
        Ok(StoredArtifact {
            sha256,
            storage_key: relative,
            byte_size: payload.len() as u64,
            compression: "none",
        })
    }

    pub fn get(&self, artifact: &StoredArtifact) -> Result<Vec<u8>> {
        let path = self.root.join(&artifact.storage_key);
        let compressed = fs::read(path)?;
        Ok(zstd::decode_all(compressed.as_slice())?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_is_content_addressed_and_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path()).unwrap();
        let bytes = b"fake-webp-bytes";
        let first = store.put_blob(bytes, "webp").unwrap();
        let second = store.put_blob(bytes, "webp").unwrap();
        assert_eq!(first.sha256, second.sha256);
        assert_eq!(first.storage_key, second.storage_key);
        assert!(first.storage_key.starts_with("assets/"));
        assert!(first.storage_key.ends_with(".webp"));
        assert_eq!(
            std::fs::read(dir.path().join("raw").join(&first.storage_key)).unwrap(),
            bytes
        );
        assert_eq!(first.compression, "none");
    }
}
