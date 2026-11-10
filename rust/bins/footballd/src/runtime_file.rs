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

pub fn write_runtime_file(port: u16) -> Result<RuntimeFile> {
    let dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("nincs felhasználói adatkönyvtár"))?
        .join("FootballAppV2");
    fs::create_dir_all(&dir)?;
    let path: PathBuf = dir.join("runtime.json");
    let runtime = RuntimeFile {
        port,
        read_token: new_token(),
        ops_token: new_token(),
    };
    let temp = path.with_extension("tmp");
    fs::write(&temp, serde_json::to_vec_pretty(&runtime)?)?;
    fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))?;
    fs::rename(temp, path)?;
    Ok(runtime)
}
