use crate::config_utils::VaultConfig;
pub use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

/// Metadata tuple format: (access_time, modified_time, file_size_bytes)
/// Modified time should be identical and latency with networks can cause different times
/// Even with a straight copy
/// vault_id is used to make sure one syncs with the correct vault
#[derive(Serialize, Deserialize, Debug)]
pub struct RemoteFile {
    pub full_path: PathBuf,
    pub root_directory: String,
    pub contents: Vec<u8>,
    pub vault_id: i32,
    pub file_id: i32,
}

impl RemoteFile {
    pub fn new(path: PathBuf, root_dir: String, vault_id: i32, file_id: i32) -> Self {
        RemoteFile {
            full_path: path.clone(),
            root_directory: root_dir,
            contents: fs::read(path).unwrap(),
            vault_id,
            file_id,
        }
    }

    /// Meant for testing of code
    pub fn new_empty(path: PathBuf, root_dir: String, vault_id: i32, file_id: i32) -> Self {
        RemoteFile {
            full_path: path.clone(),
            root_directory: root_dir,
            contents: vec![],
            vault_id,
            file_id,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ServerPresent {
    Yes,
    No,
    Unknown,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MetadataBlob {
    pub vaults: HashMap<i32, VaultMetadata>,
}

impl MetadataBlob {
    pub fn convert_to_metadata_vec(self) -> Vec<FileMetadata> {
        let mut files = Vec::with_capacity(self.vaults.len() * 5);
        for vault in self.vaults {
            for file in vault.1.files {
                if !files.contains(&file) {
                    files.push(file);
                }
            }
        }
        files
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MetadataDiff {
    pub new_for_server: HashMap<i32, VaultMetadata>,
    pub new_for_client: HashMap<i32, VaultMetadata>,
}

impl MetadataDiff {
    pub fn destruct_into_tuple(self) -> (MetadataBlob, MetadataBlob) {
        (
            MetadataBlob {
                vaults: self.new_for_client,
            },
            MetadataBlob {
                vaults: self.new_for_server,
            },
        )
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VaultMetadata {
    pub files: Vec<FileMetadata>,
    pub vault_id: i32,
}

impl VaultMetadata {
    /// Returns a new VaultMetadata tuple, 0 idx is new for client, 1st is new for server
    pub fn get_differences_from_server(
        &self,
        server: &VaultMetadata,
    ) -> (VaultMetadata, VaultMetadata) {
        let mut new_for_client = VaultMetadata {
            files: vec![],
            vault_id: server.vault_id,
        };

        let mut new_for_server = VaultMetadata {
            files: vec![],
            vault_id: server.vault_id,
        };

        for client_file in self.files.iter() {
            if client_file.present_on_server == ServerPresent::No {
                new_for_server.files.push(client_file.clone());
                continue;
            }

            for server_file in server.files.iter() {
                if client_file.compare_to(&server_file) == 1 {
                    // is client file newer than server file
                    new_for_server.files.push(client_file.clone());
                } else if client_file.compare_to(&server_file) == -1 {
                    //is server file newer than client file
                    new_for_client.files.push(server_file.clone())
                }
            }
        }

        (new_for_client, new_for_server)
    }

    pub fn get_metadata_vec(&self) -> Vec<FileMetadata> {
        self.files.clone()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileMetadata {
    pub full_path: PathBuf,
    pub root_directory: String,
    pub absolute_root_dir: PathBuf,
    pub modified_time: i64,
    pub file_size: i64,
    pub vault_id: i32,
    pub file_id: i32,
    pub present_on_server: ServerPresent,
}

impl PartialEq for FileMetadata {
    fn eq(&self, other: &Self) -> bool {
        self.file_id == other.file_id
    }
}

impl FileMetadata {
    pub fn new_from_server(
        file_id: i32,
        vault_id: i32,
        file_path: PathBuf,
        absolute_root_dir: PathBuf,
        root_dir: String,
        mod_time: i64,
        file_size: i64,
    ) -> Self {
        FileMetadata {
            full_path: file_path,
            root_directory: root_dir,
            absolute_root_dir,
            modified_time: mod_time,
            file_size,
            vault_id,
            file_id,
            present_on_server: ServerPresent::Yes,
        }
    }

    pub fn new_from_client(
        full_path: PathBuf,
        root_directory: String,
        absolute_root_dir: PathBuf,
        modified_time: i64,
        file_size: i64,
        vault_id: i32,
        file_id: i32,
    ) -> Self {
        FileMetadata {
            full_path,
            root_directory,
            absolute_root_dir,
            modified_time,
            file_size,
            vault_id,
            file_id,
            present_on_server: ServerPresent::Unknown,
        }
    }

    /// Returns 1 if the calling struct is newer than the other struct
    /// Returns -1 if the calling struct is older than the other struct
    /// 0 if equal
    pub fn compare_to(&self, other: &FileMetadata) -> i32 {
        if self.modified_time > other.modified_time {
            return 1;
        }
        if self.modified_time < other.modified_time {
            return -1;
        }
        0
    }
}

/// Sorts through all files, finds the newest files for both client and server and
/// returns it in a MetadataDiff struct
/// As the client will most likely have less vaults than the server, iterate through the client vaults
/// as we are only interested in the vaults present on a particular device
pub fn get_metadata_diff(client: MetadataBlob, server: MetadataBlob) -> MetadataDiff {
    let mut metadata_diff = MetadataDiff {
        new_for_server: HashMap::new(),
        new_for_client: HashMap::new(),
    };

    let client_vaults = client.vaults;
    for client_vault in client_vaults.into_iter() {
        let vault_id = client_vault.0;
        let server_vault = server.vaults.get(&vault_id).unwrap();
        let (client_differences, server_differences) =
            client_vault.1.get_differences_from_server(server_vault);
        metadata_diff
            .new_for_client
            .insert(vault_id, client_differences);
        metadata_diff
            .new_for_server
            .insert(vault_id, server_differences);
    }

    metadata_diff
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

/// Convenience function to convert a MetadataBlob to a vector of FileMetadata
pub fn convert_blob_to_vec_metadata(blob: &mut MetadataBlob) -> Vec<FileMetadata> {
    let mut files = Vec::with_capacity(blob.vaults.len());

    for (_, mut vault) in &mut blob.vaults {
        files.append(&mut vault.files);
    }

    files
}

/// Used as a helper function to init the DB - ensure that all files in directories declared as vaults
/// have been read and the database has up to date metadata on initial startup
/// accepts a hashmap as param => key of vault id, value of vector of file paths.
/// Then reads from the file_system and returns a MetadataBlob to be added to the DB
pub fn read_all_local_file_metadata(vaults: HashMap<i32, Vec<FileMetadata>>) -> MetadataBlob {
    let mut blob = MetadataBlob {
        vaults: HashMap::new(),
    };

    for vault in vaults {
        //update_list_of_file_metadata(&mut vault.1);

        let new_vault = VaultMetadata {
            files: vault.1,
            vault_id: vault.0,
        };
        blob.vaults.insert(vault.0, new_vault);
    }

    blob
}

/// Takes a vec of path and file_id tuples, root_directory of vault and vault id
/// then reads the file system and creates a Vec<FileMetadata> and returns it
pub fn get_file_metadata_from_path(
    paths: Vec<(i32, PathBuf)>,
    root_dir: String,
    absolute_root_dir: PathBuf,
    vault_id: i32,
) -> Vec<FileMetadata> {
    let mut files = Vec::new();

    for path in paths {
        println!("path is: {:?}", path.1);
        let metadata =
            fs::metadata(&path.1).expect(&*format!("Error reading metadata from {:?}", path));

        let vault_id = vault_id;
        let file_path = path.clone();
        let root_directory = root_dir.clone();
        let modified_time = metadata
            .modified()
            .expect(&*format!("Error reading modified metadata from {:?}", path))
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let file_size = metadata.len() as i64;
        let file = FileMetadata {
            full_path: file_path.1,
            root_directory,
            absolute_root_dir: absolute_root_dir.clone(),
            modified_time,
            file_size,
            vault_id,
            file_id: file_path.0,
            present_on_server: match file_path.0 {
                -1 => ServerPresent::No,
                _ => ServerPresent::Yes,
            },
        };
        files.push(file);
    }

    files
}

//temp function to convert all current paths in the db into their respective local paths
pub fn convert_all_paths(
    files: &Vec<PathBuf>,
    remote_root: &PathBuf,
    local_root: &PathBuf,
) -> Vec<PathBuf> {
    let mut converted_paths = Vec::with_capacity(files.len());
    for file in files {
        converted_paths.push(convert_path_to_local(file, remote_root, local_root));
    }
    converted_paths
}

/// Strips path prefixes at the root directory to get the absolute path for the client and/or server
/// Expects remote_file to be the file itself, remote root to be the rootpath of the directory and
/// local root to be the root path for the local system.
/// eg: remote_file = /home/root_dir/example_dir/file.txt
///     remote_root = /home/root_dir
///     local_root = /home/different_root_dir
/// will return /home/different_root_dir/example_dir/file.txt
pub fn convert_path_to_local(
    remote_file: &PathBuf,
    remote_root: &PathBuf,
    local_root: &PathBuf,
) -> PathBuf {
    let relative = remote_file.strip_prefix(remote_root).expect(&*format!(
        "Error stripping prefix of {:?} with {:?} - are the paths different",
        remote_file, remote_root
    ));
    local_root.join(relative)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_convert_path_to_local() {
        let file = PathBuf::from("/home/root_dir/example_dir/file.txt");
        let src = PathBuf::from("/home/root_dir");
        let dst = PathBuf::from("/home/different_root_dir");

        let result = convert_path_to_local(&file, &src, &dst);

        assert_eq!(
            result,
            PathBuf::from("/home/different_root_dir/example_dir/file.txt")
        );
    }

    fn test_get_file_metadata_from_path() {
        let vault_id = 0;
        let path = String::from("common/test_resources/metadata/test_update_file_metadata.txt");
        let path_buf = PathBuf::from(path);
        let root_dir = String::from("common/test_resources/metadata/");
        fs::File::create(&path_buf).unwrap();
        let metadata = fs::metadata(&path_buf).unwrap();

        let modified_time = metadata
            .modified()
            .expect(&*format!(
                "Error reading modified metadata from {:?}",
                &path_buf
            ))
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let file_size = metadata.len() as i64;

        let file = FileMetadata {
            full_path: path_buf.clone(),
            root_directory: root_dir.clone(),
            absolute_root_dir: Default::default(),
            modified_time,
            file_size,
            vault_id: 0,
            file_id: 0,
            present_on_server: ServerPresent::Yes,
        };

        std::thread::sleep(Duration::from_secs(1));
        fs::write(&path_buf, "some text").unwrap();
        let now_modified = fs::metadata(&path_buf)
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
    }
}

/*
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


 */
