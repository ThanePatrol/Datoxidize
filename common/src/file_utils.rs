use std::{fs};
use std::error::Error;
use std::path::{PathBuf};
use std::time::{SystemTime};
use filetime::FileTime;
pub use serde::{Deserialize, Serialize};
use crate::config_utils::{DirectoryConfig, VaultConfig};

//todo - add file_id
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

/// Metadata tuple format: (access_time, modified_time, file_size_bytes)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemoteFileMetadata {
    pub full_path: PathBuf,
    pub root_directory: String,
    pub metadata: (SystemTime, SystemTime, u64),
    pub vault_id: i32,
    pub file_id: i32,
}

impl RemoteFileMetadata {
    pub fn new(path: PathBuf, root_dir: String, vault_id: i32) -> Self {
        let metadata = fs::metadata(path.clone()).unwrap();
        RemoteFileMetadata {
            full_path: path,
            root_directory: root_dir,
            metadata: (metadata.accessed().unwrap(), metadata.modified().unwrap(), metadata.len()),
            vault_id,
            file_id: -1,
        }
    }
}

/// Returns a tuple of Vecs, the 0th item contains the vec of files that are more recent on client
/// The 1st item contains a vec of files that are more recent on the server
pub fn get_list_of_newer_files(
    client: &Vec<RemoteFileMetadata>,
    server: &Vec<RemoteFileMetadata>) -> (Vec<RemoteFileMetadata>, Vec<RemoteFileMetadata>) {

    let mut client_new = Vec::new();
    let mut server_new = Vec::new();



    (client_new, server_new)
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

pub async fn copy_file_to_local(server_file: &RemoteFile,
                                directory_config: &DirectoryConfig)
    -> Result<(), Box<dyn Error>> {

    Ok(())
}

//todo - implement diff_copy to only sync differences
/// saves the file to the server, if the directory is not present, create it
pub async fn copy_file_to_server(client_file: &RemoteFile, vault_config: &VaultConfig) -> Result<(), Box<dyn Error>> {
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

pub fn get_all_file_metadata(path: &PathBuf, vault_config: &VaultConfig) -> Vec<RemoteFileMetadata> {
    let file_paths = get_all_files_from_path(path)
        .expect(&*format!("Error reading path: {}", path.display()));

    let mut files = Vec::with_capacity(file_paths.len());

    for file_path in file_paths {
        let file = RemoteFileMetadata::new(
            file_path.clone(),
            vault_config.vault_root.clone(),
            vault_config.vault_id);
        files.push(file)
    }
    files
}
