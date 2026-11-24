use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeFile {
    pub port: u16,
    pub read_token: String,
    pub ops_token: String,
}

pub fn new_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn reuse_or_new(previous: Option<&RuntimeFile>, port: u16) -> RuntimeFile {
    RuntimeFile {
        port,
        read_token: previous
            .map(|p| p.read_token.clone())
            .unwrap_or_else(new_token),
        ops_token: previous
            .map(|p| p.ops_token.clone())
            .unwrap_or_else(new_token),
    }
}

pub fn write_runtime_file(port: u16) -> Result<RuntimeFile> {
    let dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("nincs felhasználói adatkönyvtár"))?
        .join("FootballAppV2");
    fs::create_dir_all(&dir)?;
    let path: PathBuf = dir.join("runtime.json");
    let previous: Option<RuntimeFile> = fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    let runtime = reuse_or_new(previous.as_ref(), port);
    let temp = path.with_extension("tmp");
    fs::write(&temp, serde_json::to_vec_pretty(&runtime)?)?;
    fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))?;
    fs::rename(temp, path)?;
    Ok(runtime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_tokens_survive_restart() {
        let old = RuntimeFile {
            port: 1234,
            read_token: "read-token".into(),
            ops_token: "ops-token".into(),
        };
        let next = reuse_or_new(Some(&old), 4321);
        assert_eq!(next.port, 4321);
        assert_eq!(next.read_token, old.read_token);
        assert_eq!(next.ops_token, old.ops_token);

        let fresh = reuse_or_new(None, 4321);
        assert_ne!(fresh.read_token, fresh.ops_token);
        assert!(!fresh.read_token.is_empty());
    }
}
