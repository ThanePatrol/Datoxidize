use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
pub use serde::{Deserialize, Serialize};


/// Metadata tuple format: (access_time, modified_time, file_size_bytes)
/// Modified time should be identical and latency with networks can cause different times
/// Even with a straight copy
/// vault_id is used to make sure one syncs with the correct vault
#[derive(Serialize, Deserialize)]
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

    pub fn get_contents(file: &Self) -> &Vec<u8> {
        &file.contents
    }

    pub fn get_metadata(file: &Self) -> (SystemTime, SystemTime, u64) {
        file.metadata
    }

    pub fn get_vault_id(file: &Self) -> i32 {
        file.vault_id
    }

    pub fn get_root_dir(file: &Self) -> String {
        file.root_directory.clone()
    }

}