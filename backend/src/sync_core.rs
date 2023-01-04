use std::collections::HashMap;
use std::{fs};
use std::path::{Path, PathBuf};
use once_cell::sync::Lazy;
use common::file_utils::{copy_file_to_server, FileMetadata, get_server_path, MetadataBlob, VaultMetadata};
use common::{file_utils, RemoteFile};
use common::config_utils::{deserialize_vault_config, VaultConfig};


/// Used as a helper function to init the DB - ensure that all files in directories declared as vaults
/// have been read and the database has up to date metadata on initial startup
/// accepts a hashmap as param => key of vault id, value of vector of file paths.
/// Then reads from the file_system and returns a MetadataBlob to be added to the DB
fn read_all_local_file_metadata(vaults: HashMap<i32, Vec<FileMetadata>>) -> MetadataBlob {
    let mut blob = MetadataBlob {
        vaults: HashMap::new(),
    };

    //todo - where i got up to: Populate the DB with updated file metadata on client launch
    // once loaded, start back and forward communication with server
    for mut vault in vaults {

        file_utils::update_list_of_file_metadata(&mut vault.1);


    }

    blob
}


/*-----------------------------OLD STUFF BELOW-----------------------------------------*/









static VAULT_CONFIGS: Lazy<HashMap<i32, VaultConfig>> = Lazy::new(|| {
        deserialize_vault_config()
});

//pub async fn send_list_of_newer_files_to_client(local_files: Vec<RemoteFile>, )

/// Returns true if sender has a more recent copy of a file than local
pub fn is_client_more_recent_than_server(remote: &RemoteFile, local: &PathBuf) -> bool {
    if !Path::new(local).exists() {
        return true
    }
    let mdata_err_msg = "Can't read metadata on this platform";

    let server_metadata = fs::metadata(local)
        .expect(&*format!("Error reading metadata from {:?}", local));

    let client_time = remote.metadata.1;
    let server_time = server_metadata.modified()
        .expect(mdata_err_msg);

    client_time > server_time
}

//todo - set metadata from local to payload
//todo log error messages rather than unwrapping
pub async fn sync_file_with_server(payload: RemoteFile) -> bool {
    let vaults = &VAULT_CONFIGS;
    let config = vaults.get(&payload.vault_id)
        .expect("Vault_id not found");
    let server_file_path = get_server_path(&payload, &config);
    println!("Path: {}", server_file_path.display());

    if is_client_more_recent_than_server(&payload, &server_file_path) {
        copy_file_to_server(&payload, &config).await.unwrap();
        true
    } else {
        false
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use common::config_utils::deserialize_vault_config;
    use std::time::SystemTime;

    #[test]
    fn test_get_server_path() {
        let file = RemoteFile::new_empty(
            PathBuf::from("../datoxidize/example_dir/lophostemon_occurrences.csv"),
        "example_dir".to_string(),
        0,
        );

        let vault = deserialize_vault_config();
        let path = get_server_path(&file, &vault.get(&0).unwrap());
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


}