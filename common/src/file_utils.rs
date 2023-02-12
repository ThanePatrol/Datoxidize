use rayon::prelude::*;
pub use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{UNIX_EPOCH};

/// Metadata tuple format: (access_time, modified_time, file_size_bytes)
/// Modified time should be identical and latency with networks can cause different times
/// Even with a straight copy
/// vault_id is used to make sure one syncs with the correct vault
#[derive(Serialize, Deserialize, Debug)]
pub struct RemoteFile {
    pub full_path: PathBuf,
    pub root_directory: String,
    pub absolute_root_dir: PathBuf,
    pub contents: Vec<u8>,
    pub vault_id: i32,
    pub file_id: i32,
    pub modified_time: i64,
}

impl RemoteFile {
    pub fn new(
        path: PathBuf,
        absolute_root_dir: PathBuf,
        root_dir: String,
        vault_id: i32,
        file_id: i32,
        modified_time: i64,
    ) -> Self {
        RemoteFile {
            full_path: path.clone(),
            root_directory: root_dir,
            absolute_root_dir,
            contents: fs::read(path).unwrap(),
            vault_id,
            file_id,
            modified_time,
        }
    }

    /// Meant for testing of code
    pub fn new_empty(path: PathBuf, root_dir: String, vault_id: i32, file_id: i32) -> Self {
        RemoteFile {
            full_path: path.clone(),
            root_directory: root_dir,
            absolute_root_dir: Default::default(),
            contents: vec![],
            vault_id,
            file_id,
            modified_time: 0,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ServerPresent {
    Yes,
    No,
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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

    /// Convenience function for printing a pretty string from a Metadata Diff struct
    pub fn get_pretty_string(&self) -> (String, String) {
        let mut client_contents = String::from("client: ");
        let mut server_contents = String::from("server: ");

        for entry in self.new_for_client.iter() {
            for vault in &entry.1.files {
                client_contents.push_str("path { ");
                client_contents.push_str(vault.full_path.to_str().unwrap());
                client_contents.push_str(" }, modified { ");
                client_contents.push_str(vault.modified_time.to_string().as_str());
                client_contents.push_str(" }, ");
            }
        }

        for entry in self.new_for_server.iter() {
            for vault in &entry.1.files {
                server_contents.push_str("path { ");
                server_contents.push_str(vault.full_path.to_str().unwrap());
                server_contents.push_str(" }, modified { ");
                server_contents.push_str(vault.modified_time.to_string().as_str());
                server_contents.push_str(" }");
            }
        }
        (client_contents, server_contents)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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
            //if client_file.present_on_server == ServerPresent::No {
            //    new_for_server.files.push(client_file.clone());
            //    continue;
            //}
            let mut present = false;
            let client_path_in_server_format = convert_path_to_local(
                &client_file.full_path,
                &client_file.absolute_root_dir,
                &server.files[0].absolute_root_dir);


            for server_file in server.files.iter() {
                //make sure we are comparing same file
                if client_path_in_server_format == server_file.full_path {
                    println!("comparing {:#?} and {:#?}", client_file, server_file);
                    // is client file newer than server file
                    if client_file.compare_to(&server_file) == 1 {
                        new_for_server.files.push(client_file.clone());
                    }
                    //is server file newer than client file
                    else if client_file.compare_to(&server_file) == -1 {
                        new_for_client.files.push(server_file.clone())
                    }

                    present = true;
                }
            }
            if !present {
                new_for_server.files.push(client_file.clone());
            }
        }


        // Checks if file_id matches any client files, if not the client needs it
        for server_file in server.files.iter() {
            let mut present = false;
            let server_path_in_client_format = convert_path_to_local(
                &server_file.full_path,
                &server_file.absolute_root_dir,
                &self.files[0].absolute_root_dir);

            for client_file in self.files.iter() {
                if server_path_in_client_format == client_file.full_path {
                    present = true;
                }
            }
            if !present {
                new_for_client.files.push(server_file.clone());
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

//todo unit test me!!!!!
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

    for (_, vault) in &mut blob.vaults {
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
        "Error stripping prefix of {:?} with {:?} - are the paths different?",
        remote_file, remote_root
    ));
    local_root.join(relative)
}

/// Goes through a vec of remote files, converts their path to work on the local system
/// and saves to disk
/// done in parallel for greater speed

pub fn save_remote_files_to_disk(files: Vec<RemoteFile>, id_and_root_dirs: Vec<(i32, PathBuf)>) {
    /// Loops through all the vec of (vault_id, root_path) until the vault id of the file
    /// is matched, then the root_path for that vault is returned
    fn get_local_root(file: &RemoteFile, id_and_dirs: &Vec<(i32, PathBuf)>) -> PathBuf {
        let mut local_root = &Default::default();
        for (id, local_r) in id_and_dirs.iter() {
            if *id == file.vault_id {
                local_root = local_r;
                break;
            }
        }
        local_root.clone()
    }

    let iter = files.into_par_iter();
    let _ = iter.for_each(|file| {
        let local_root = get_local_root(&file, &id_and_root_dirs);

        let local_path =
            convert_path_to_local(&file.full_path, &file.absolute_root_dir, &local_root);
        fs::write(&local_path, file.contents)
            .expect(&*format!("Error writing {} to disk", local_path.display()));

        set_modified_time(&local_path, file.modified_time);
    });
}

/// Update the metadata to ensure file won't be synced unnecessarily
fn set_modified_time(path: &PathBuf, modified_time: i64) {
    println!("set {:?} modified time to {modified_time}", path);
    filetime::set_file_mtime(
        path,
        filetime::FileTime::from_unix_time(modified_time, 0),
    )
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_path_to_local() {
        let file = PathBuf::from("/home/root_dir/nested_dir/file.txt");
        let src = PathBuf::from("/home/root_dir");
        let dst = PathBuf::from("/home/different_root_dir");

        let result = convert_path_to_local(&file, &src, &dst);

        assert_eq!(
            result,
            PathBuf::from("/home/different_root_dir/nested_dir/file.txt")
        );
    }

    #[test]
    fn test_check_metadata_difference() {

        let client_mdata = VaultMetadata {
            files: vec![
                FileMetadata {
                    full_path: PathBuf::from("/home/sync_dir/nested/memes1.txt"),
                    root_directory: "sync_dir".to_string(),
                    absolute_root_dir: PathBuf::from("/home/sync_dir/"),
                    modified_time: 1_000_000_000_000,
                    file_size: 10,
                    vault_id: 0,
                    file_id: 1,
                    present_on_server: ServerPresent::Yes,
                },
                FileMetadata {
                    full_path: PathBuf::from("/home/sync_dir/nested/memes2.txt"),
                    root_directory: "sync_dir".to_string(),
                    absolute_root_dir: PathBuf::from("/home/sync_dir/"),
                    modified_time: 2_000_000_000_000,
                    file_size: 10,
                    vault_id: 0,
                    file_id: -1,
                    present_on_server: ServerPresent::No,
                },
            ],
            vault_id: 0,
        };

        let server_mdata = VaultMetadata {
            files: vec![FileMetadata {
                full_path: PathBuf::from("/other_home/sync_dir/nested/memes1.txt"),
                root_directory: "sync_dir".to_string(),
                absolute_root_dir: PathBuf::from("/other_home/sync_dir/"),
                modified_time: 1_000_000_000_001,
                file_size: 10,
                vault_id: 0,
                file_id: 1,
                present_on_server: ServerPresent::Yes,
            }, FileMetadata {
                full_path: PathBuf::from("/other_home/sync_dir/nested/memes3.txt"),
                root_directory: "sync_dir".to_string(),
                absolute_root_dir: PathBuf::from("/other_home/sync_dir/"),
                modified_time: 2_000_000_000_000,
                file_size: 10,
                vault_id: 0,
                file_id: 2,
                present_on_server: ServerPresent::Yes,
            }],
            vault_id: 0,
        };

        let client_metadata_blob = MetadataBlob {
            vaults: HashMap::from([(0, client_mdata)]),
        };

        let server_metadata_blob = MetadataBlob {
            vaults: HashMap::from([(0, server_mdata)]),
        };

        let diff = get_metadata_diff(client_metadata_blob, server_metadata_blob);
        let (client, server) = diff.get_pretty_string();

        assert!(client.contains("memes1.txt"));
        assert!(client.contains("memes3.txt"));
        assert!(server.contains("memes2.txt"));
    }
}

