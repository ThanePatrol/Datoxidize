use std::collections::{HashMap, HashSet};
use std::fs::Metadata;
use std::io::{Read, Write};
use std::path::{PathBuf};
use std::time::{Duration, SystemTime};
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
    pub directory_id: i32,
    pub sync_frequency: Duration,
    pub ignored_files: HashSet<PathBuf>,
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
            ignored_files: HashSet::new(),
        }
    }
}

/// Public API that reads through directories and syncs files and directories
pub fn initial_sync(directory: &DirectoryConfig) {

    init_sync_local_to_remote(directory);
    init_sync_remote_to_local(directory);

}

//todo - create a method called get_full_local_path that takes a remote file and a config and gets an appropriate local file path - needed for bidirectional syncing
fn init_sync_remote_to_local(config: &DirectoryConfig) {
    fn get_list_of_files_to_update_on_local(local_metadata: &Vec<(PathBuf, Metadata)>, remote_meta_data: &Vec<(PathBuf, Metadata)>) -> Vec<PathBuf> {
        let mut to_sync = Vec::new();
        let local = convert_paths_to_hashmap(local_metadata);
        let remote = convert_paths_to_hashmap(remote_meta_data);

        for remote_file in remote.iter() {
            if local.contains_key(remote_file.0) {
                let local_time = local.get(remote_file.0).unwrap().1;
                let time_diff;
                match remote_file.1.1.duration_since(local_time) {
                    Ok(res) => time_diff = res,
                    Err(_) => continue,
                }

                if time_diff > Duration::from_secs(5) {
                    to_sync.push(remote_file.1.0.to_owned());
                }
            } else {
                to_sync.push(remote_file.1.0.to_owned());
            }
        }
        to_sync
    }
    fn copy_remote_changes_to_local(files_to_copy: Vec<PathBuf>, config: &DirectoryConfig) {
        let file_copy_options = fs_extra::file::CopyOptions {
            overwrite: true,
            skip_exist: false,
            ..Default::default()
        };

        for file in files_to_copy {
            let save_path = get_full_local_path(&file.to_str().unwrap().to_string(), &config);
            println!("save path: {}, source: {:?}", save_path, file);
            fs_extra::file::copy(file, save_path, &file_copy_options).unwrap();
        }
    }

    let local_data = get_files_and_metadata(config, true);
    let remote_data = get_files_and_metadata(config, false);
    let files_to_sync = get_list_of_files_to_update_on_local(&local_data, &remote_data);
    println!("files to sync: {:?}", files_to_sync);
    copy_remote_changes_to_local(files_to_sync, config);
}

fn init_sync_local_to_remote(config: &DirectoryConfig) {
    fn get_list_of_files_to_update_on_remote (local_metadata: &Vec<(PathBuf, Metadata)>, remote_metadata: &Vec<(PathBuf, Metadata)>) -> Vec<PathBuf> {
        let mut to_sync = Vec::new();
        let local = convert_paths_to_hashmap(local_metadata);
        let remote = convert_paths_to_hashmap(remote_metadata);
        for file in local.iter() {
            if remote.contains_key(file.0) {
                // file is present on the remote is the modification time greater than 5 seconds? is what this code is checking
                let remote_time = remote.get(file.0).unwrap();
                let time_diff;
                let _ = match file.1.1.duration_since(remote_time.1) {
                    Ok(res) => time_diff = res,
                    Err(_) => continue,
                };

                if time_diff > Duration::from_secs(5) {
                    to_sync.push(file.1.0.to_owned());
                }

            } else {
                to_sync.push(file.1.0.to_owned())
            }
        }
        to_sync
    }

    /// Takes a Vec<PathBuf> of the updated and new local files then copies them to remote
    fn copy_local_changes_from_local_to_remote(files: Vec<PathBuf>, directory: &DirectoryConfig ) {
        let file_copy_options = fs_extra::file::CopyOptions {
            overwrite: true,
            skip_exist: false,
            ..Default::default()
        };

        for file in files {
            let save_path = get_full_remote_path(&file.to_str().unwrap().to_string(), &directory);
            fs_extra::file::copy(&file, &save_path, &file_copy_options).expect(&*format!("Error try to save {:?} to {}", file, save_path));
        }
    }

    let local_data = get_files_and_metadata(config, true);
    let remote_data = get_files_and_metadata(config, false);
    let files_to_sync = get_list_of_files_to_update_on_remote(&local_data, &remote_data);
    copy_local_changes_from_local_to_remote(files_to_sync, config);
}

/// Returns a hashmap with filenames as the key and the value as a tuple containing the full path and the time of last modification
fn convert_paths_to_hashmap(files: &Vec<(PathBuf, Metadata)>) -> HashMap<PathBuf, (PathBuf, SystemTime)> {
    let mut paths_and_modifications = HashMap::new();
    for file in files {
        paths_and_modifications.insert(PathBuf::from(file.0.clone().file_name().unwrap()), (file.0.clone(), file.1.modified().unwrap()));
    }
    paths_and_modifications
}

/// Read through files in local root directory, get the metadata of each file unless file is specifically ignored
fn get_files_and_metadata(directory_config: &DirectoryConfig, local_flag: bool) -> Vec<(PathBuf, Metadata)> {
    let mut metadata = Vec::new();
    let dir_content;
    if local_flag {
        dir_content = fs_extra::dir::get_dir_content(&directory_config.content_directory).unwrap();
    } else {
        dir_content = fs_extra::dir::get_dir_content(&directory_config.remote_relative_directory).unwrap()
    }

    for file in dir_content.files {
        let path = PathBuf::from(&file);
        if directory_config.ignored_files.contains(&*PathBuf::from(path.file_name().unwrap())) {
            continue
        }
        let data = (path, std::fs::metadata(&file).expect(&*format!("Trying to read, {}", &file)));
        metadata.push(data);
    }
    metadata
}

/// Main public api for syncing a changed file to a remote dir
/// Takes an event and the directorySettings that the event corresponds to an syncs it with the remote
pub fn sync_changed_file(event: &Vec<PathBuf>, directory: &DirectoryConfig) {
    for single_change in event {
        println!("single change: {:?}", single_change);
        let data_to_sync = std::fs::read(single_change.as_path())
            .expect("Error reading data to sync");

        let remote_path = get_full_remote_path(
            &single_change.as_os_str().to_str().unwrap().to_string(),
            directory
        );

        std::fs::write(&remote_path, data_to_sync)
            .unwrap_or_else(|_| panic!("Error syncing data for {remote_path}"));
    }
}

/// Main public API for creating a folder
pub fn create_folder_on_remote(events: &Vec<PathBuf>, directory: &DirectoryConfig) {
    for single_change in events {
        let folder_to_add = get_full_remote_path(
            &single_change.as_os_str().to_str().unwrap().to_string(), directory);

        let path = PathBuf::from(folder_to_add);
        std::fs::create_dir_all(path).expect("Error creating directory");
    }
}

/// Main public API for deleting files and folders
pub fn remove_files_and_dirs_from_remote(events: &Vec<PathBuf>, directory: &DirectoryConfig) {
    for single_change in events {
        let file_to_remove = get_full_remote_path(
            &single_change.as_os_str().to_str().unwrap().to_string(), directory);

        let path = PathBuf::from(file_to_remove);
        if path.is_file() {
            let _ = match std::fs::remove_file(&path) {
                Ok(_r) => println!("Removed file {:?} successfully", &path),
                Err(_e) => println!("Error removing file {:?}, not found on remote", &path),
            };
        } else {
            let _ = match std::fs::remove_dir(&path) {
                Ok(_r) => println!("Removed directory {:?} successfully", &path),
                Err(_e) => println!("Error removing directory {:?}, not found on remote", &path),
            };
        }
    }
}

/// Main API for getting the local path to sync something to from a remote path
fn get_full_local_path(event_path: &String, directory: &DirectoryConfig) -> String {
    fn build_local_remote_path(directory: &DirectoryConfig) -> String {
        directory.content_directory.clone()
    }
    fn build_local_directory_structure(remote_file_path: &String, directory: &DirectoryConfig) -> String {
        let remote_path = remote_file_path.clone();
        let pattern = remove_path_approximate_from_config(&mut directory.content_directory.clone());
        let split_path = remote_path.rsplit_once(pattern.as_str()).unwrap();
        split_path.1.to_string()
    }

    let mut path = String::new();
    let root = build_local_remote_path(directory);
    path.push_str(root.as_str());
    let dir_structure = build_local_directory_structure(event_path, directory);
    path.push_str(dir_structure.as_str());
    path
}

/// Main API for building the remote path for any file or directory syncing
/// Builds full directory as a string for a specific file or directory
/// Takes the event_path of the local event and a directory config
fn get_full_remote_path(event_path: &String, directory: &DirectoryConfig) -> String {
    /// Will build the remote_storage path, eg: ./dir1/content_dir/
    /// this still needs the directory structure from the local appended
    fn build_root_remote_path(directory: &DirectoryConfig) -> String {
        let mut remote_path = dotenvy::var("ROOT_STORAGE").unwrap();
        remote_path.push_str("dir");
        remote_path.push_str(&directory.directory_id.to_string());
        remote_path.push('/');
        let parent_dir = remove_path_approximate_from_config(&mut directory.content_directory.clone());
        remote_path.push_str(&parent_dir);
        remote_path
    }

    let mut path = String::new();
    let root = build_root_remote_path(directory);
    path.push_str(root.as_str());
    let dir_structure = build_remote_directory_structure(&event_path, &directory.content_directory);
    path.push_str(dir_structure.as_str());
    path
}

/// Builds directory structure for the remote path by removing all the local path stuff
/// until the directory_root name is found
/// NB as rsplit_once finds the last occurrence of the given &str and splits it there, one cannot
/// have the same folder name
fn build_remote_directory_structure(event_path: &String, content_directory_root: &String) -> String {
    let mut new_path = String::new();
    let full_event = event_path.clone();
    let pattern = remove_path_approximate_from_config(&mut content_directory_root.clone());
    let split_path = full_event.rsplit_once(pattern.as_str()).unwrap();
    new_path.push_str(split_path.1);
    new_path
}

/// config stores a path in a `./dir` manner, the slash and dot need to be removed
fn remove_path_approximate_from_config(relative_dir: &mut String) -> String {
    if relative_dir.starts_with("./") {
        relative_dir.remove(0);
        relative_dir.remove(0);
    }
    relative_dir.to_string()
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
    use std::thread;
    use rand::distributions::{Alphanumeric, Standard};
    use rand::{Rng, SeedableRng};
    use rand::rngs::StdRng;
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_build_directory_structure() {
        let local_path = String::from("./example_dir/test_build_dir/test");
        let config = deserialize_config("./test_resources/config.json".to_string()).unwrap();

        let full_path = get_full_remote_path(&local_path, &config);

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
        let full_path = get_full_remote_path(&local_path, &config);
        local_path.remove(0);

        //what the final remote path should be
        let mut remote_path = String::from("./copy_dir/dir1");
        remote_path.push_str(local_path.as_str());
        assert_eq!(remote_path, full_path)
    }

    #[test]
    fn test_sync_changed_file() {
        // copy files from test dir into watched dir
        let paths = vec![
            "p1.csv",
            "tester/test_sync_remove.csv",
        ];
        copy_test_items_into_watched_dir(&paths);

        let event_paths = vec![
            PathBuf::from_str("./example_dir/p1.csv").unwrap(),
            PathBuf::from_str("./example_dir/tester/test_sync_remove.csv").unwrap()
        ];
        let config = deserialize_config("./test_resources/config.json".to_string()).unwrap();

        sync_changed_file(&event_paths, &config);

        let p1 = Path::new("./copy_dir/dir1/example_dir/p1.csv");
        let nested = Path::new("./copy_dir/dir1/example_dir/tester/test_sync_remove.csv");
        assert!(p1.exists());
        assert!(nested.exists());

        std::fs::remove_file(p1).unwrap();
        std::fs::remove_file(nested).unwrap();
        delete_test_items_from_watched_dir(&paths)
    }

    #[test]
    #[serial]
    fn test_remove_synced_file() {
        let paths = vec![
            "p3.csv",
            "p4.csv",
            "test/test_sync.csv",
        ];
        copy_test_items_into_watched_dir(&paths);


        let local_event_paths = vec![
            PathBuf::from_str("./example_dir/p3.csv").unwrap(),
            PathBuf::from_str("./example_dir/p4.csv").unwrap(),
            PathBuf::from_str("./example_dir/test/test_sync.csv").unwrap()
        ];
        let config = deserialize_config("./test_resources/config.json".to_string()).unwrap();
        sync_changed_file(&local_event_paths, &config);

        let p3 = Path::new("./copy_dir/dir1/example_dir/p3.csv");
        let nested = Path::new("./copy_dir/dir1/example_dir/test/test_sync.csv");
        assert!(p3.exists());
        assert!(nested.exists());

        remove_files_and_dirs_from_remote(&local_event_paths, &config);

        assert!(!p3.exists());
        assert!(!nested.exists());

        delete_test_items_from_watched_dir(&paths);
    }

    /// Checks if a single directory is deleted
    #[test]
    fn test_remove_synced_directory() {
        let mut root_local = "./example_dir/test".to_string();

        for i in 0..5 {
            if i % 5 == 0 {
                root_local.push('/');
                continue
            }
            let rand_str: String = (0..7).map(|_|
                                          rand::thread_rng()
                                              .sample(Alphanumeric) as char)
                .collect();


            root_local.push_str(rand_str.as_str());
        }
        let local_path = root_local;
        let config = deserialize_config("./test_resources/config.json".to_string()).unwrap();
        let remote_path_str = get_full_remote_path(
            &local_path,
            &config
        );
        let remote_path = Path::new(remote_path_str.as_str());


        std::fs::create_dir_all(remote_path).unwrap();
        assert!(remote_path.exists());
        assert!(remote_path.is_dir());

        remove_files_and_dirs_from_remote(
            &vec![PathBuf::from_str(remote_path_str.as_str()).unwrap()],
            &config
        );
        assert!(!remote_path.exists());

    }

    #[test]
    fn test_get_files_with_durations() {
        fs_extra::dir::create_all("./example_dir/tea/", false).unwrap();
        fs_extra::dir::create_all("./copy_dir/dir1/example_dir/tea", false).unwrap();
        let root_str = "./example_dir/tea/";
        let memes_str = "./example_dir/tea/memes.txt";
        let memes2_str = "./example_dir/tea/memes2.txt";

        let root = Path::new(root_str);
        std::fs::File::create(memes_str).unwrap();

        thread::sleep(Duration::from_secs(2));
        std::fs::File::create(memes2_str).expect(&*format!("Error creating {}", memes2_str));

        let dir_config = deserialize_config("./test_resources/config.json".to_string()).unwrap();

        //This is the main thing being test, is the metadata collection accurate
        let metadata = get_files_and_metadata(&dir_config, true);

        println!("{:?}", metadata);
        let meme_path = PathBuf::from(memes_str);
        let meme2_path = PathBuf::from(memes2_str);

        let mut data_of_interest = Vec::new();
        for data in metadata {
            if data.0 == meme_path {
                data_of_interest.push(data)
            } else if data.0 == meme2_path {
                data_of_interest.push(data)
            }
        }


        let memes1_time = data_of_interest[0].1.created().unwrap();
        let memes2_time = data_of_interest[1].1.created().unwrap();
        let difference = memes2_time.duration_since(memes1_time).unwrap();

        assert!(difference.gt(&Duration::from_secs(1)));

        std::fs::remove_file(memes_str).unwrap();
        std::fs::remove_file(memes2_str).unwrap();
        fs_extra::dir::remove("./example_dir/tea").unwrap();
        fs_extra::dir::remove("./copy_dir/dir1/example_dir/tea").unwrap();
    }

    /// Test if ignored files in config are being respected
    #[test]
    fn test_are_ignored_files_being_ignored() {
        let config = deserialize_config("./test_resources/config.json".to_string()).unwrap();
        let metadata = get_files_and_metadata(&config, true);

        let should_not_be_synced = config.ignored_files;

        for file in metadata {
            assert!(!should_not_be_synced.contains(&file.0))
        }
    }

    #[test]
    #[serial]
    fn test_copy_files_from_local_to_remote_if_not_present() {
        let config = deserialize_config("./test_resources/config.json".to_string()).unwrap();
        let file_origin = vec!["./example_dir/op3.csv", "./example_dir/op4.csv"];
        for f in file_origin.iter() {
            std::fs::File::create(f).unwrap();
        }

        init_sync_local_to_remote(&config);

        let files_remote = vec!["./copy_dir/dir1/example_dir/op3.csv", "./copy_dir/dir1/example_dir/op4.csv"];

        for file in files_remote {
            let path = Path::new(file);
            println!("{:?}", path);
            assert!(path.exists());
            std::fs::remove_file(path).unwrap();
        }

        for file in file_origin {
            std::fs::remove_file(file).unwrap();
        }
    }

    #[test]
    #[serial]
    fn test_copy_files_from_remote_to_local_if_not_present() {
        thread::sleep(Duration::from_secs(1));
        let config = deserialize_config("./test_resources/config.json".to_string()).unwrap();
        let file_origin = vec!["./copy_dir/dir1/example_dir/op1.csv", "./copy_dir/dir1/example_dir/op2.csv"];
        for f in file_origin.iter() {
            std::fs::File::create(f).unwrap();
        }

        init_sync_remote_to_local(&config);
        let files_local = vec!["./example_dir/op1.csv", "./example_dir/op2.csv"];
        for file in files_local {
            let path = Path::new(file);
            assert!(path.exists());
            std::fs::remove_file(path).unwrap();
        }

        for file in file_origin {
            std::fs::remove_file(file).unwrap();
        }
    }

    fn copy_test_items_into_watched_dir(paths: &Vec<&str>) {
        let copy_options = fs_extra::file::CopyOptions {
            overwrite: true,
            skip_exist: false,
            ..Default::default()
        };
        fs_extra::dir::create_all("./example_dir/test", false).unwrap();
        fs_extra::dir::create_all("./copy_dir/dir1/example_dir/test", false).unwrap();
        fs_extra::dir::create_all("./example_dir/tester", false).unwrap();
        fs_extra::dir::create_all("./copy_dir/dir1/example_dir/tester", false).unwrap();
        fs_extra::dir::create_all("./example_dir/t", false).unwrap();
        fs_extra::dir::create_all("./copy_dir/dir1/example_dir/t", false).unwrap();

        for p in paths {
            let mut path = String::from("./test_resources/random_test_files/");
            path.push_str(p);
            println!("path: {}", path);
            let mut save_path = String::from("./example_dir/");
            save_path.push_str(p);
            println!("save path: {}", save_path);
            fs_extra::file::copy(path, save_path, &copy_options).unwrap();
        }
    }

    fn delete_test_items_from_watched_dir(paths: &Vec<&str>) {
        for file in paths {
            let mut path = String::from("./example_dir/");
            path.push_str(file);
            std::fs::remove_file(path).unwrap();
        }
        fs_extra::dir::remove("./example_dir/test").unwrap();
        fs_extra::dir::remove("./copy_dir/dir1/example_dir/test").unwrap();
        fs_extra::dir::remove("./example_dir/tester").unwrap();
        fs_extra::dir::remove("./copy_dir/dir1/example_dir/tester").unwrap();
        fs_extra::dir::remove("./example_dir/t").unwrap();
        fs_extra::dir::remove("./copy_dir/dir1/example_dir/t").unwrap();

    }
    /*
    #[test]
    fn test_newer_files_on_remote_are_not_overwritten() {
        let config = deserialize_config("./test_resources/config.json".to_string()).unwrap();

    }

     */

    //todo - create tests with nested folders on local that are not present on remote. Are those folders created where appropriate
}