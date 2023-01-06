use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::{env, fs};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;
use serde::{Deserialize, Serialize};
use std::io::Write;

#[derive(Serialize, Deserialize, Clone)]
pub struct VaultConfig {
    pub full_path: PathBuf,
    pub vault_id: i32,
    pub sync_frequency: Duration,
    pub vault_root: String,
}

/// Deserializes a vault relative to the working directory of where it is called from
/// If called from the backend it will be looking for a directory of ./backend/resources/vault_config.json
/// This allows for different test configs and production configs to be retrieved from the same address
pub fn deserialize_vault_config() -> HashMap<i32, VaultConfig> {
    println!("resource dir: {:?}", env::current_dir());
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
        vault_root: "example_dir".to_string(),
    })]);
    let ser = serde_json::to_string(&test_vaults).unwrap();
    fs::write(Path::new("./resources/vault_config.json"), &ser).unwrap()
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryConfig {
    pub content_directory: String,
    pub remote_relative_directory: String,
    pub directory_id: i32,
    pub sync_frequency: Duration,
    pub ignored_files: HashSet<PathBuf>,
}

impl DirectoryConfig {
    pub fn _new(content_directory: String, directory_id: i32, sync_frequency: Duration) -> Self {
        let mut remote_relative_directory = dotenvy::var("ROOT_STORAGE").unwrap();
        remote_relative_directory.push_str("dir");
        remote_relative_directory.push_str(directory_id.to_string().as_str());
        remote_relative_directory.push('/');
        DirectoryConfig {
            content_directory,
            remote_relative_directory,
            directory_id,
            sync_frequency,
            ignored_files: HashSet::new(),
        }
    }
}

pub fn _serialize_config_settings(config: &HashMap<i32, DirectoryConfig>, path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let serial = serde_json::to_string(config).unwrap();
    let mut file = fs::File::create(path)?;
    write!(file, "{}", serial).expect("Error serializing config");
    Ok(())
}

pub fn deserialize_config(path: &PathBuf) -> Result<HashMap<i32, DirectoryConfig>, Box<dyn Error>> {
    let mut json = String::new();
    fs::File::open(path)?.read_to_string(&mut json)?;
    Ok(serde_json::from_str(&json).unwrap())
}
