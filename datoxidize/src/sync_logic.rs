use std::collections::{HashMap, HashSet};
use std::fs::Metadata;
use std::io::{Read, Write};
use std::path::{PathBuf};
use std::time::{Duration, SystemTime};
use fs_extra::file::CopyOptions;
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
/// Should only be done on startup
pub fn initial_sync(directory: &DirectoryConfig) {
    fn get_list_of_files_to_update_on_remote (local_metadata: &Vec<(PathBuf, Metadata)>, remote_metadata: &Vec<(PathBuf, Metadata)>) -> Vec<PathBuf> {
        let mut to_sync = Vec::new();
        let local = get_files_with_modified_time(local_metadata);
        let remote = get_files_with_modified_time(remote_metadata);
        for file in local.iter() {
            if remote.contains_key(file.0) {
                let remote_time = *remote.get(file.0).unwrap();
                let time_diff;
                let _ = match remote_time.elapsed() {
                    Ok(res) => time_diff = res,
                    Err(_) => time_diff = file.1.elapsed().unwrap(),
                };
                if time_diff > Duration::from_secs(5) {
                    to_sync.push(file.0.to_owned());
                }
                //println!(remote.)

            } else {
                println!(" file not present on remote: {:?}", file.0);
                to_sync.push(file.0.to_owned())
            }
        }
        to_sync
    }

    fn get_files_with_modified_time(files: &Vec<(PathBuf, Metadata)>) -> HashMap<PathBuf, SystemTime> {
        let mut paths_and_modifications = HashMap::new();
        for file in files {
            paths_and_modifications.insert(file.0.clone(), file.1.modified().unwrap());
        }
        paths_and_modifications
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
            println!("save path: {}, source: {:?}", save_path, file);
            fs_extra::file::copy(file, save_path, &file_copy_options).unwrap();
        }
    }

    let local_data = get_files_and_metadata(directory, true);
    let remote_data = get_files_and_metadata(directory, false);
    let files_to_sync = get_list_of_files_to_update_on_remote(&local_data, &remote_data);

    println!("files to sync: {:?}", files_to_sync);
    copy_local_changes_from_local_to_remote(files_to_sync, directory);


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
        let data = (path, std::fs::metadata(file).unwrap());
        metadata.push(data);
    }
    metadata
}

/// Main public api for syncing a changed file to a remote dir
/// Takes an event and the directorySettings that the event corresponds to an syncs it with the remote
pub fn sync_changed_file(event: &Vec<PathBuf>, directory: &DirectoryConfig) {
    for single_change in event {
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


/// Main API for building the remote path for any file or directory syncing
/// Builds full directory as a string for a specific file or directory
/// Takes the event_path of the local event and a directory config
fn get_full_remote_path(event_path: &String, directory: &DirectoryConfig) -> String {
    let mut path = String::new();
    let root = build_root_remote_path(directory);
    path.push_str(root.as_str());
    let dir_structure = build_directory_structure(&event_path, &directory.content_directory);
    path.push_str(dir_structure.as_str());
    path
}

/// Builds directory structure for the remote path by removing all the local path stuff
/// until the directory_root name is found
/// NB as rsplit_once finds the last occurrence of the given &str and splits it there, one cannot
/// have the same folder name
fn build_directory_structure(event_path: &String, content_directory_root: &String) -> String {
    let mut new_path = String::new();
    let full_event = event_path.clone();
    let pattern = remove_path_approximate_from_config(&mut content_directory_root.clone());
    let split_path = full_event.rsplit_once(pattern.as_str()).unwrap();
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
    let parent_dir = remove_path_approximate_from_config(&mut directory.content_directory.clone());
    remote_path.push_str(&parent_dir);
    remote_path
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

    #[test]
    fn test_remove_synced_file() {
        let local_event_paths = vec![
            PathBuf::from_str("./example_dir/Multimedia.tsv").unwrap(),
            PathBuf::from_str("./example_dir/test/test_sync_remove.csv").unwrap()
        ];
        let config = deserialize_config("./test_resources/config.json".to_string()).unwrap();
        sync_changed_file(&local_event_paths, &config);

        let taxon = Path::new("./copy_dir/dir1/example_dir/Multimedia.tsv");
        let nested = Path::new("./copy_dir/dir1/example_dir/test/test_sync_remove.csv");
        assert!(taxon.exists());
        assert!(nested.exists());
        remove_files_and_dirs_from_remote(&local_event_paths, &config);

        assert!(!taxon.exists());
        assert!(!nested.exists());
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
        let root_str = "./example_dir/test";
        let memes_str = "./example_dir/test/memes.txt";
        let memes2_str = "./example_dir/test/memes2.txt";

        let root = Path::new(root_str);
        std::fs::create_dir_all(root).unwrap();
        std::fs::File::create(memes_str).unwrap();

        thread::sleep(Duration::from_secs(2));
        std::fs::File::create(memes2_str).unwrap();

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
    fn test_copy_files_from_local_to_remote_if_not_present() {
        let config = deserialize_config("./test_resources/config.json".to_string()).unwrap();

        let file_origin = vec!["./test_resources/random_test_files/p1.csv", "./test_resources/random_test_files/p2.csv"];
        let copy_options = fs_extra::dir::CopyOptions {
            overwrite: true,
            skip_exist: false,
            ..Default::default()
        };
        fs_extra::copy_items(&file_origin, "./example_dir", &copy_options).unwrap();

        initial_sync(&config);

        let files_remote = vec!["./copy_dir/dir1/example_dir/p1.csv", "./copy_dir/dir1/example_dir/p2.csv"];

        for file in files_remote {
            let path = Path::new(file);
            println!("{:?}", path);
            assert!(path.exists());
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_file("./example_dir/p1.csv").unwrap();
        std::fs::remove_file("./example_dir/p2.csv").unwrap();

    }

    //todo - create tests with nested folders on local that are not present on remote. Are those folders created where appropriate
}