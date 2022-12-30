use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct VaultConfig {
    pub full_path: PathBuf,
    pub vault_id: i32,
    pub sync_frequency: Duration,
}

pub fn deserialize_vault_config() -> HashMap<i32, VaultConfig> {
    let mut json = String::new();
    fs::File::open("./resources/vault_config.json")
        .expect(&*format!("Vault config not found at {:?}", std::env::current_dir().unwrap()))
        .read_to_string(&mut json)
        .expect("Error reading vault config");
    let str = json.as_str();
    serde_json::from_str(str).expect("Error in format of vault_config.json")
}

fn _create_vault_config() {
    let test_vaults = HashMap::from([(0, VaultConfig {
        full_path: PathBuf::from("./storage/"),
        vault_id: 0,
        sync_frequency: Duration::from_secs(5),
    })]);
    let ser = serde_json::to_string(&test_vaults).unwrap();
    fs::write(Path::new("./resources/vault_config.json"), &ser).unwrap()
}