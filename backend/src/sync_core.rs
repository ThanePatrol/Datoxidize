use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::fs::{Metadata};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use axum::{
    extract,
};
use filetime::FileTime;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

static  VAULT_CONFIGS: Lazy<HashMap<i32, VaultConfig>> = Lazy::new(|| {
        deserialize_vault_config()
    });

/// Metadata tuple format: (access_time, modified_time, file_size_bytes)
/// Modified time should be identical and latency with networks can cause different times
/// Even with a straight copy
/// vault_id is used to make sure one syncs with the correct vault
#[derive(Serialize, Deserialize)]
pub struct RemoteFile {
    pub(crate) full_path: PathBuf,
    pub(crate) root_directory: String,
    pub(crate) contents: Vec<u8>,
    pub(crate) metadata: (SystemTime, SystemTime, u64),
    pub(crate) vault_id: i32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct VaultConfig {
    full_path: PathBuf,
    vault_id: i32,
    sync_frequency: Duration,
}

/*fn get_relative_path(client: &PathBuf, server: &PathBuf) -> PathBuf {
    pathdiff::diff_paths(client, server)
        .expect(&*format!("Called get_relative_path on client {:?} and server {:?}", client, server))
}

 */


/// Returns true if client has a more recent copy of a file
fn is_client_more_recent_than_server(client: &RemoteFile, server: &PathBuf) -> bool {
    if !is_file_exists(server) {
        return true
    }
    let mdata_err_msg = "Can't read metadata on this platform";

    let server_metadata = std::fs::metadata(server)
        .expect(&*format!("Error reading metadata from {:?}", server));

    let client_time = client.metadata.1;
    let server_time = server_metadata.modified()
        .expect(mdata_err_msg);

    client_time > server_time
}

fn is_file_exists(path: &PathBuf) -> bool {
   Path::new(path).exists()
}


//todo - set metadata from local to payload
//todo log error messages rather than unwrapping
pub async fn sync_file_with_server(payload: RemoteFile) -> bool{
    let vaults = &VAULT_CONFIGS;
    let config = vaults.get(&payload.vault_id)
        .expect("Vault_id not found");
    let server_file_path = get_server_path(&payload, &config);
    println!("Path: {}", server_file_path.display());

    if is_client_more_recent_than_server(&payload, &server_file_path) {
        copy_file(&payload, &config).await.unwrap();
        true
    } else {
        false
    }
}

//todo - implement diff_copy to only sync differences
/// saves the file to the server, if the directory is not present, create it
async fn copy_file(client_file: &RemoteFile, vault_config: &VaultConfig) -> Result<(), Box<dyn Error>> {
    let full_path = get_server_path(client_file, vault_config);
    println!("full path: {:?}", full_path.clone());
    //todo - handle parent option error by logging
    let directory = full_path.parent().unwrap();

    fs::create_dir_all(directory)?;
    fs::write(&full_path, &client_file.contents)?;

    filetime::set_file_times(
        full_path,
        FileTime::from_system_time(*&client_file.metadata.0),
        FileTime::from_system_time(*&client_file.metadata.1))?;

    Ok(())
}

fn get_server_path(client: &RemoteFile, vault: &VaultConfig) -> PathBuf {
    fn build_server_dir_structure(client: &RemoteFile) -> String {
        let vault_root = client.root_directory.clone();
        client.full_path
            .clone()
            .as_os_str()
            .to_str()
            .expect(&*format!("Error casting {:?} to string", client.full_path))
            .to_string()
            .rsplit_once(vault_root.as_str())
            .expect("Pattern not found")
            .1
            .to_string()
    }

    let mut path = String::from("./storage/");
    path.push_str("vault");
    path.push_str(&vault.vault_id.to_string());
    path.push_str(build_server_dir_structure(client).as_str());

    PathBuf::from(path)
}

fn deserialize_vault_config() -> HashMap<i32, VaultConfig> {
    let mut json = String::new();
    std::fs::File::open("./resources/vault_config.json")
        .expect("Vault config not found")
        .read_to_string(&mut json)
        .expect("Error reading vault config");
    let str = json.as_str();
    serde_json::from_str(str).expect("Error in format of vault_config.json")
}

fn create_vault_config() {
    let test_vaults = HashMap::from([(0, VaultConfig {
        full_path: PathBuf::from("./storage/"),
        vault_id: 0,
        sync_frequency: Duration::from_secs(5),
    })]);
    let ser = serde_json::to_string(&test_vaults).unwrap();
    fs::write(Path::new("./resources/vault_config.json"), &ser).unwrap()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_server_path() {
        let client = RemoteFile {
            full_path: PathBuf::from("../datoxidize/example_dir/lophostemon_occurrences.csv"),
            root_directory: "example_dir".to_string(),
            contents: vec![],
            metadata: (SystemTime::now(), SystemTime::now(), 0),
            vault_id: 0,
        };
        let vault = deserialize_vault_config();
        let path = get_server_path(&client, &vault.get(&0).unwrap());
        assert_eq!(path.as_os_str().to_str().unwrap(), "./storage/vault0/lophostemon_occurrences.csv");
    }

    #[test]
    fn test_get_nested_server_path() {
        let client = RemoteFile {
            full_path: PathBuf::from("../datoxidize/example_dir/serve_test/another/test.txt"),
            root_directory: "example_dir".to_string(),
            contents: vec![],
            metadata: (SystemTime::now(), SystemTime::now(), 0),
            vault_id: 0,
        };
        let vault = deserialize_vault_config();
        let path = get_server_path(&client, &vault.get(&0).unwrap());
        assert_eq!(path.as_os_str().to_str().unwrap(), "./storage/vault0/serve_test/another/test.txt")
    }

    #[test]
    fn is_client_more_recent_than_server() {

    }

    /*
    #[test]
    fn create_dummy_server_config() {
        create_vault_config();
        assert!(Path::new("./resources/vault_config.json").exists())
    }

     */


}