use std::{fs};
use std::collections::HashMap;
use std::error::Error;
use std::path::{PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use filetime::FileTime;
use rayon::prelude::*;
pub use serde::{Deserialize, Serialize};
use crate::config_utils::{VaultConfig};

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
pub struct MetadataDiff {
    pub new_for_server: HashMap<i32, VaultMetadata>,
    pub new_for_client: HashMap<i32, VaultMetadata>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VaultMetadata {
    pub files: Vec<FileMetadata>,
    pub vault_id: i32,
}

impl VaultMetadata {
    /// Returns a new VaultMetadata tuple, 0 idx is new for client, 1st is new for server
    pub fn get_differences_from_server(&self, server: &VaultMetadata) -> (VaultMetadata, VaultMetadata) {
        let mut new_for_client = VaultMetadata {
            files: vec![],
            vault_id: server.vault_id,
        };

        let mut new_for_server = VaultMetadata {
            files: vec![],
            vault_id: server.vault_id,
        };

        for client_file in self.files.iter() {
            // if file_id is -1 then file is not present on server
            if client_file.file_id == -1 {
                new_for_server.files.push(client_file.clone());
                continue;
            }

            for server_file in server.files.iter() {

                if client_file.compare_to(&server_file) == 1 { // is client file newer than server file
                    new_for_server.files.push(client_file.clone());
                } else if client_file.compare_to(&server_file) == -1 { //is server file newer than client file
                    new_for_client.files.push(server_file.clone())
                }
            }
        }

        (new_for_client, new_for_server)
    }

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

    /// Returns 1 if the calling struct is newer than the other struct
    /// Returns -1 if the calling struct is older than the other struct
    /// 0 if equal
    pub fn compare_to(&self, other: &FileMetadata) -> i32 {
        if self.modified_time > other.modified_time { return 1; }
        if self.modified_time < other.modified_time { return -1; }
        0
    }
}


/// Sorts through all files, finds the newest files for both client and server and
/// returns it in a MetadataDiff struct
/// As the client will most likely have less vaults than the server, iterate through the client vaults
pub fn get_metadata_diff(
    client: MetadataBlob,
    server: MetadataBlob) -> MetadataDiff {

    let mut metadata_diff = MetadataDiff {
        new_for_server: HashMap::new(),
        new_for_client: HashMap::new(),
    };


    let client_vaults = client.vaults;
    for client_vault in client_vaults.into_iter() {
        let vault_id = client_vault.0;
        let server_vault = server.vaults.get(&vault_id).unwrap();
        let differences = client_vault.1.get_differences_from_server(server_vault);
        metadata_diff.new_for_client.insert(vault_id, differences.0);
        metadata_diff.new_for_server.insert(vault_id, differences.1);
    }

    metadata_diff
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



/// Used as a helper function to init the DB - ensure that all files in directories declared as vaults
/// have been read and the database has up to date metadata on initial startup
/// accepts a hashmap as param => key of vault id, value of vector of file paths.
/// Then reads from the file_system and returns a MetadataBlob to be added to the DB
pub fn read_all_local_file_metadata(vaults: HashMap<i32, Vec<FileMetadata>>) -> MetadataBlob {
    let mut blob = MetadataBlob {
        vaults: HashMap::new(),
    };

    for mut vault in vaults {

        update_list_of_file_metadata(&mut vault.1);

        let new_vault = VaultMetadata {
            files: vault.1,
            vault_id: vault.0,
        };
        blob.vaults.insert(vault.0, new_vault);
    }

    blob
}

//todo - make this only update files if they're changed
fn update_list_of_file_metadata(files: &mut Vec<FileMetadata>) {
    files.par_iter_mut().for_each(|file| update_file_metadata(file));
}

/// Used for updating a the metadata of a file. Useful for the initial startup of a client and/or server
fn update_file_metadata(file: &mut FileMetadata) {
    let metadata = fs::metadata(&file.full_path)
        .expect(&*format!("Error reading metadata from {:?}", file.full_path));

    file.modified_time = metadata.modified().unwrap().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    file.file_size = metadata.len() as i64;
}

/// Takes a vec of path and file_id tuples, root_directory of vault and vault id
/// then reads the file system and creates a Vec<FileMetadata> and returns it
pub fn get_file_metadata_from_path_client(paths: Vec<(i32, PathBuf)>, root_dir: String, vault_id: i32) -> Vec<FileMetadata> {
    let mut files = Vec::new();

    for path in paths {
        let metadata = fs::metadata(&path.1)
            .expect(&*format!("Error reading metadata from {:?}", path));

        let vault_id = vault_id;
        let file_path = path.clone();
        let root_directory = root_dir.clone();
        let modified_time = metadata
            .modified()
            .expect(&*format!("Error reading modified metadata from {:?}", path))
            .duration_since(UNIX_EPOCH).unwrap()
            .as_secs() as i64;
        let file_size = metadata.len() as i64;
        let file = FileMetadata {
            full_path: file_path.1,
            root_directory,
            modified_time,
            file_size,
            vault_id,
            file_id: file_path.0,
        };
        files.push(file);
    }

    files
}



/*---------------------------------------TO DELETE -------------------------------------*/

/// Test function for getting metadata populated from a particular path
pub fn test_get_file_metadata_from_path_client(paths: Vec<PathBuf>) -> Vec<FileMetadata> {
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