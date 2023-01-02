use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use axum::http;
use common::RemoteFile;
use http::StatusCode;
use reqwest::Url;
use common::config_utils::{DirectoryConfig, VaultConfig};

pub async fn init_sync(local_configs: &HashMap<i32, DirectoryConfig>, url: &Url) -> StatusCode {
    let remote_configs = get_remote_config(url).await;

    let _ = send_local_files_to_remote();

    StatusCode::OK
}

async fn get_remote_config(url: &Url) -> Result<HashMap<i32, VaultConfig>, Box<dyn Error>> {
    let mut config_url = url.clone();
    config_url.set_path("config/all");

    let config  = reqwest::get(config_url)
        .await?
        .json::<HashMap<i32, VaultConfig>>()
        .await?;
    Ok(config)

}

async fn sync_remote_files_to_local(files: Vec<RemoteFile>) {

}

async fn send_local_files_to_remote() -> StatusCode {
    let local_files = get_list_of_files_for_updating();
    let client = reqwest::Client::new();
    let mut status = StatusCode::OK;
    for file in local_files {
        let response = client
            .post("http://localhost:8080/copy")
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
fn get_list_of_files_for_updating() -> Vec<RemoteFile>{
    let mut files = Vec::new();
    let path = PathBuf::from("./client/example_dir");
    let file_paths = common::file_utils::get_all_files_from_path(&path).unwrap();

    for path in file_paths {
        let file = RemoteFile::new(path, "example_dir".to_string(), 0);
        files.push(file);
    }
    files
}

