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
    pub fn new(content_directory: String, directory_id: i32, sync_frequency: Duration) -> Self {
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

pub fn serialize_config_settings(config: &DirectoryConfig, path: String) -> Result<()> {
    let mut serial = serde_json::to_string(config).unwrap();
    let mut file = std::fs::File::create(path)?;
    write!(file, "{}", serial).expect("Error serializing config");
    Ok(())
}

pub fn deserialize_config(path: String) -> Result<DirectoryConfig> {
    let mut json = String::new();
    std::fs::File::open(path)?.read_to_string(&mut json)?;
    Ok(serde_json::from_str(&json).unwrap())
}

/// Takes an event and the directorySettings that the event corresponds to an syncs it with the remote
pub fn sync_changed_file(event: Event, directory: &DirectoryConfig) {
    println!("{event:?}");
    let file_name = event.paths[0].file_name().expect("Error reading source file path");
    let data_to_sync = std::fs::read(event.paths[0].as_path())
        .expect("Error reading data to sync");
    let mut remote_path = build_generic_remote_path(directory);
    remote_path.push_str(file_name.to_str().unwrap());

    println!("file name: {:?}", file_name);
    println!("remote path: {}", remote_path);
    std::fs::write(remote_path, data_to_sync).expect("Error syncing data")
}

fn build_generic_remote_path(directory: &DirectoryConfig) -> String {
    let mut remote_path = dotenvy::var("ROOT_STORAGE").unwrap();
    remote_path.push_str("dir");
    remote_path.push_str(&directory.directory_id.to_string());
    remote_path.push('/');
    remote_path.push_str(&directory.content_directory);
    remote_path.push('/');
    remote_path
}

pub fn get_new_remote_directory_path(event_path: String, directory: &DirectoryConfig) -> String {
    let mut new_dir_path = String::from(directory.remote_relative_directory.clone());
    new_dir_path.push_str(
        get_relative_string_path(
            event_path.as_str()
                    ,directory).as_str());

    new_dir_path
}

pub fn create_new_remote_directory(path: String) {
    std::fs::create_dir(path).expect("Error creating directory on remote");
}

pub fn remove_file_from_remote(event: Event, directory: &DirectoryConfig) {

}

/// Gets the relative path based upon sync root
/// eg if user syncs the /home/user/Documents directory which contains the folder /stuff
/// then we want the relative file path of a file in the Documents directory eg /stuff/memes.txt
/// this is so we can sync it on the server as ~/dir1/stuff/memes.txt
fn get_relative_string_path(root_path: &str, directory: &DirectoryConfig) -> String {
    let new_dir = &root_path[root_path
        .find(directory.content_directory.as_str())
        .unwrap()
        ..root_path.len()];

    new_dir.to_string()
}