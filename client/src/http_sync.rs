use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use axum::http;
use common::RemoteFile;
use http::StatusCode;
use reqwest::{Client, Url};
use common::config_utils::{DirectoryConfig, VaultConfig};
use common::file_utils::{MetadataBlob, FileMetadata};

/// Main api that is called on launch of client
/// Will make request to server for a list of all files and their metadata
/// Once received, go through the list of files, if there is something more recent on server
/// It makes a request for that file, if the file is more recent on the client, send it to server
pub async fn init_sync(url: Url) -> Result<(), Box<dyn Error>> {

    let client = Client::new();
    let mut endpoint = url.clone();
    endpoint.set_path("/copy/metadata_blob");
    let server_metadata = get_metadata_from_server(&client, endpoint).await?;
    println!("metadata blo=b: {:?}", server_metadata);

    Ok(())
}

/// Gets the every file and its update time from server
async fn get_metadata_from_server(client: &Client, url: Url) -> Result<MetadataBlob, Box<dyn Error>> {
    client
        .get(url)
        .send()
        .await?
        .json()
        .await?
}

async fn send_init_list_of_local_files() -> StatusCode {
    let local_files = vec![get_list_of_files_for_updating()];
    println!("size of vec: {}", local_files.len());
    let client = reqwest::Client::new();
    client
        .post("http://localhost:3000/copy/init")
        .json(&local_files)
        .send()
        .await
        .unwrap()
        .status()
}

async fn get_remote_config(url: &Url) -> Result<HashMap<i32, VaultConfig>, Box<dyn Error>> {
    let mut config_url = url.clone();
    config_url.set_path("config/all");

    let config = reqwest::get(config_url)
        .await?
        .json::<HashMap<i32, VaultConfig>>()
        .await?;
    Ok(config)
}

async fn sync_remote_files_to_local(files: Vec<RemoteFile>) {}

async fn send_local_files_to_remote() -> StatusCode {
    let local_files = get_list_of_files_for_updating();
    let client = reqwest::Client::new();
    let mut status = StatusCode::OK;
    for file in local_files {
        let response = client
            .post("http://localhost:3000/copy/init")
            .json(&file)
            .send()
            .await
            .unwrap();

        if response.status().is_server_error() {
            status = response.status();
        }
    }
    status
}

//todo - make a struct that simply has the file metadata, not the entire file
fn get_list_of_files_for_updating() -> Vec<FileMetadata> {
    let mut files = Vec::new();
    let path = PathBuf::from("./client/example_dir");
    let file_paths = common::file_utils::get_all_files_from_path(&path).unwrap();

    for path in file_paths {
        let file = FileMetadata::new(path, "example_dir".to_string(), 0);
        files.push(file);
    }
    files
}

