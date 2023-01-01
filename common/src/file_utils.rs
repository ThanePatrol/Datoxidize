use std::{fs};
use std::error::Error;
use std::path::{PathBuf};
use std::time::{SystemTime};
use filetime::FileTime;
pub use serde::{Deserialize, Serialize};
use crate::vault_utils::VaultConfig;


/// Metadata tuple format: (access_time, modified_time, file_size_bytes)
/// Modified time should be identical and latency with networks can cause different times
/// Even with a straight copy
/// vault_id is used to make sure one syncs with the correct vault
#[derive(Serialize, Deserialize, Debug)]
pub struct RemoteFile {
    pub full_path: PathBuf,
    pub root_directory: String,
    pub contents: Vec<u8>,
    pub metadata: (SystemTime, SystemTime, u64),
    pub vault_id: i32,
}

impl RemoteFile {
    pub fn new(path: PathBuf, root_dir: String, vault_id: i32) -> Self {
        let metadata = fs::metadata(path.clone()).unwrap();
        RemoteFile {
            full_path: path.clone(),
            root_directory: root_dir,
            contents: fs::read(path).unwrap(),
            metadata: (metadata.accessed().unwrap(), metadata.modified().unwrap(), metadata.len()),
            vault_id,
        }
    }

    /// Meant for testing of code
    pub fn new_empty(path: PathBuf, root_dir: String, vault_id: i32) -> Self {
        RemoteFile {
            full_path: path.clone(),
            root_directory: root_dir,
            contents: vec![],
            metadata: (SystemTime::UNIX_EPOCH, SystemTime::UNIX_EPOCH, 0),
            vault_id,
        }
    }
}



pub fn get_server_path(client: &RemoteFile, vault: &VaultConfig) -> PathBuf {
    fn build_server_dir_structure(client: &RemoteFile) -> String {
        let vault_root = client.root_directory.clone();
        client.full_path
            .clone()
            .as_os_str()
            .to_str()
            .expect(&*format!("Error casting {:?} to string", client.full_path))
            .to_string()
            .rsplit_once(vault_root.as_str())
            .expect(&*format!("Pattern {} not found", vault_root))
            .1
            .to_string()
    }

    let mut path = String::from("./storage/");
    path.push_str("vault");
    path.push_str(&vault.vault_id.to_string());
    path.push_str(build_server_dir_structure(client).as_str());

    PathBuf::from(path)
}

//todo - implement diff_copy to only sync differences
/// saves the file to the server, if the directory is not present, create it
pub async fn copy_file(client_file: &RemoteFile, vault_config: &VaultConfig) -> Result<(), Box<dyn Error>> {
    let full_path = get_server_path(client_file, vault_config);
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

pub fn get_all_files_from_path(path: &PathBuf) -> std::io::Result<Vec<PathBuf>> {
    fn recursive_walk(path: &PathBuf, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let cur_path = entry.path();

            if entry.file_type()?.is_dir() {
                recursive_walk(&cur_path, files)?;
            } else {
                files.push(cur_path);
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    recursive_walk(path, &mut files)?;
    Ok(files)
}
