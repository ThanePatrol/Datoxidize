use std::{fs};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::path::{PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
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

#[derive(Serialize, Deserialize, Debug)]
pub struct MetadataBlob {
    pub vaults: HashMap<i32, VaultMetadata>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VaultMetadata {
    pub files: Vec<FileMetadata>,
    pub vault_id: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileMetadata {
    pub full_path: PathBuf,
    pub root_directory: String,
    pub modified_time: i64,
    pub file_size: i64,
    pub vault_id: i32,
    pub file_id: i32,
}

impl FileMetadata {
    pub fn new_from_server(
        file_id: i32,
        vault_id: i32,
        file_path: PathBuf,
        root_dir: String,
        mod_time: i64,
        file_size: i64,
    ) -> Self {
        FileMetadata {
            full_path: file_path,
            root_directory: root_dir,
            modified_time: mod_time,
            file_size,
            vault_id,
            file_id,
        }
    }

    pub fn new_from_client(
        full_path: PathBuf,
        root_directory: String,
        modified_time: i64,
        file_size: i64,
        vault_id: i32,
        file_id: i32,
    ) -> Self {
        FileMetadata {
            full_path,
            root_directory,
            modified_time,
            file_size,
            vault_id,
            file_id,
        }
    }
}

/// Returns a tuple of Vecs, the 0th item contains the vec of files that are more recent on client
/// The 1st item contains a vec of files that are more recent on the server
pub fn get_list_of_newer_files(
    client: &Vec<FileMetadata>,
    server: &Vec<FileMetadata>) -> (Vec<FileMetadata>, Vec<FileMetadata>) {
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

/// Convenience function to read all files in all subdirs of a supplied path
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

/// Test function for getting metadata populated from a particular path
pub fn get_file_metadata_from_path_client(paths: Vec<PathBuf>) -> Vec<FileMetadata> {
    let mut files = Vec::new();

    for path in paths {
        let vault_id = 0;
        let file_path = path.clone();
        let root_directory = "example_dir".to_string();
        let modified_time = fs::metadata(&path).unwrap().modified().unwrap().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        let file_size = fs::metadata(&path).unwrap().len() as i64;
        let file = FileMetadata {
            full_path: file_path,
            root_directory,
            modified_time,
            file_size,
            vault_id,
            file_id: -1,
        };
        files.push(file);
    }

    files
}

