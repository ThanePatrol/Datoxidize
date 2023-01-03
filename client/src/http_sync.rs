use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use axum::http;
use common::RemoteFile;
use http::StatusCode;
use reqwest::Url;
use common::config_utils::{DirectoryConfig, VaultConfig};
use common::file_utils::RemoteFileMetadata;

pub async fn init_sync(local_configs: &HashMap<i32, DirectoryConfig>, url: &Url) -> StatusCode {
    //let remote_configs = get_remote_config(url).await;

    let reponse = send_init_list_of_local_files().await;

    println!("sent to remote with response of {:?}", reponse);

    StatusCode::OK
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
fn get_list_of_files_for_updating() -> Vec<RemoteFileMetadata> {
    let mut files = Vec::new();
    let path = PathBuf::from("./client/example_dir");
    let file_paths = common::file_utils::get_all_files_from_path(&path).unwrap();

    for path in file_paths {
        let file = RemoteFileMetadata::new(path, "example_dir".to_string(), 0);
        files.push(file);
    }
    files
}

