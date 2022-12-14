use notify::*;

/// root_directory specifies the directory for the syncing to occur
/// This will be mirrored locally and remote
/// Remote will have "dir{directory_id}/" appended to the front of the path
/// where directory_id is a unique i32
/// Sync frequency is specified
pub struct DirectorySettings {
    root_directory: String,
    directory_id: i32,
    sync_frequency_in_seconds: i32,
}

impl DirectorySettings {
    pub fn new(root_directory: String, directory_id: i32, sync_frequency_in_seconds: i32) -> Self {
        DirectorySettings {
            root_directory,
            directory_id,
            sync_frequency_in_seconds
        }
    }
}

//takes an event and the directorySettings that the event corresponds to an syncs it with the remote
pub fn sync_directory(event: Event, directory: &DirectorySettings) {
    let file_name = event.paths[0].file_name().expect("Error reading source file path");
    let data_to_sync = std::fs::read(event.paths[0].as_path())
        .expect("Error reading data to sync");
    let mut remote_path = dotenvy::var("ROOT_STORAGE").unwrap();
    remote_path.push_str("dir");
    remote_path.push_str(&directory.directory_id.to_string());
    remote_path.push('/');
    remote_path.push_str(&directory.root_directory);
    println!("file name: {:?}", file_name);
    println!("remote path: {}", remote_path);

}