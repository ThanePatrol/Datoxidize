use std::io::{Read, Write};
use std::time::Duration;
use notify::*;
use serde::{Deserialize, Serialize};

/// root_directory specifies the directory for the syncing to occur, this should
/// mirror the local dir exactly
/// This will be mirrored locally and remote
/// Remote will have "dir{directory_id}/" appended to the front of the path
/// where directory_id is a unique i32
/// Sync frequency is specified
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryConfig {
    pub content_directory: String,
    pub remote_relative_directory: String,
    directory_id: i32,
    sync_frequency: Duration,
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
        }
    }
}

/// Main public api for syncing a changed file to a remote dir
/// Takes an event and the directorySettings that the event corresponds to an syncs it with the remote
pub fn sync_changed_file(event: &Vec<std::path::PathBuf>, directory: &DirectoryConfig) {
    println!("{event:?}");
    for single_change in event {
        //let file_name = single_change.file_name().expect("Error reading source file path");
        let data_to_sync = std::fs::read(single_change.as_path())
            .expect("Error reading data to sync");

        let remote_path = build_full_remote_path(
            &single_change.as_os_str().to_str().unwrap().to_string(),
            directory
        );

        std::fs::write(&remote_path, data_to_sync)
            .unwrap_or_else(|_| panic!("Error syncing data for {remote_path}"));
    }
}

fn convert_event_paths(event_paths: &Vec<std::path::PathBuf>) -> Vec<String> {
    event_paths.iter().map(|path| {
        path.clone().into_os_string().into_string().unwrap()
    }).collect::<Vec<String>>()
}

pub fn remove_file_from_remote(event: Event, directory: &DirectoryConfig) {
    let mut remote_file_to_remove = build_root_remote_path(directory);
    remote_file_to_remove.push_str(event.paths[0].file_name().unwrap().to_str().unwrap());
    std::fs::remove_file(remote_file_to_remove.clone()).expect("Couldn't remove file");
    println!("removed: {remote_file_to_remove:?}")
}

//todo - convert all methods to use this method
/// Main API for building the remote path for any file or directory syncing
/// Builds full directory as a string for a specific file or directory
/// Takes the event_path of the local event and a directory config
fn build_full_remote_path(event_path: &String, directory: &DirectoryConfig) -> String {
    let mut path = String::new();
    let root = build_root_remote_path(directory);
    path.push_str(root.as_str());
    let dir_structure = build_directory_structure(&event_path, &directory.content_directory);
    println!("{dir_structure}");
    path.push_str(dir_structure.as_str());
    path
}

/// Builds directory structure for the remote path by removing all the local path stuff
/// until the directory_root name is found
/// NB as rsplit_once finds the last occurrence of the given &str and splits it there, one cannot
/// have the same folder name
fn build_directory_structure(event_path: &String, directory_root: &String) -> String {
    let mut new_path = String::new();
    let full_event = event_path.clone();
    let split_path = full_event.rsplit_once(directory_root.as_str()).unwrap();
    new_path.push_str(split_path.1);
    new_path
}

/// Will build the remote_storage path, eg: ./dir1/content_dir/
/// this still needs the directory structure from the local appended
fn build_root_remote_path(directory: &DirectoryConfig) -> String {
    let mut remote_path = dotenvy::var("ROOT_STORAGE").unwrap();
    remote_path.push_str("dir");
    remote_path.push_str(&directory.directory_id.to_string());
    remote_path.push('/');
    remote_path.push_str(&directory.content_directory);
    remote_path
}

pub fn _serialize_config_settings(config: &DirectoryConfig, path: String) -> Result<()> {
    let serial = serde_json::to_string(config).unwrap();
    let mut file = std::fs::File::create(path)?;
    write!(file, "{}", serial).expect("Error serializing config");
    Ok(())
}

pub fn deserialize_config(path: String) -> Result<DirectoryConfig> {
    let mut json = String::new();
    std::fs::File::open(path)?.read_to_string(&mut json)?;
    Ok(serde_json::from_str(&json).unwrap())
}

//todo write tests for directory creation and deletion
#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::str::FromStr;
    use rand::distributions::Standard;
    use rand::{Rng, SeedableRng};
    use rand::rngs::StdRng;
    use super::*;

    #[test]
    fn test_build_directory_structure() {
        let local_path = String::from("./example_dir/test_build_dir/test");
        let config = deserialize_config("./test_resources/config.json".to_string()).unwrap();

        let full_path = build_full_remote_path(&local_path, &config);

        assert_eq!(full_path, "./copy_dir/dir1/example_dir/test_build_dir/test".to_string())
    }

    #[test]
    fn test_build_many_random_directories() {
        let mut chars = "./example_dir/".chars().collect::<Vec<char>>();

        for i in 0..1000 {
            if i % 5 == 0 {
                chars.push('/');
                continue
            }

            let random_char: char = StdRng::from_entropy().sample(Standard);
            if *chars.last().unwrap() == '/' && random_char == '/' {
                continue
            }
            chars.push(random_char);
        }
        let mut local_path = chars.into_iter().collect::<String>();
        let config = deserialize_config("./test_resources/config.json".to_string()).unwrap();
        let full_path = build_full_remote_path(&local_path, &config);
        local_path.remove(0);

        //what the final remote path should be
        let mut remote_path = String::from("./copy_dir/dir1");
        remote_path.push_str(local_path.as_str());
        assert_eq!(remote_path, full_path)
    }

    #[test]
    fn test_sync_changed_file() {
        let event_paths = vec![
            PathBuf::from_str("./example_dir/Taxon.tsv").unwrap(),
            PathBuf::from_str("./example_dir/test/test_sync.csv").unwrap()
        ];
        let config = deserialize_config("./test_resources/config.json".to_string()).unwrap();

        sync_changed_file(&event_paths, &config);

        let taxon = Path::new("./copy_dir/dir1/example_dir/Taxon.tsv");
        let nested = Path::new("./copy_dir/dir1/example_dir/test/test_sync.csv");
        assert!(taxon.exists());
        assert!(nested.exists());

        std::fs::remove_file(taxon).unwrap();
        std::fs::remove_file(nested).unwrap();
    }
}