use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::sync::{Arc};
use tokio::sync::Barrier;
use axum::http;
use common::RemoteFile;
use http::StatusCode;
use reqwest::{Client, Url};
use sqlx::{Pool, Sqlite};
use common::config_utils::{DirectoryConfig, VaultConfig};
use common::file_utils::{MetadataBlob, FileMetadata};
use crate::client_db_api::load_file_metadata;

/// Main api that is called on launch of client
/// Will make request to server for a list of all files and their metadata
/// Once received, go through the list of files, if there is something more recent on server
/// It makes a request for that file, if the file is more recent on the client, send it to server
pub async fn init_metadata_sync(url: Url, pool: &Pool<Sqlite>) -> Result<(Client, Url, MetadataBlob, MetadataBlob, i32), sqlx::Error> {

    let client = Client::new();

    let get_metadata_url = get_metadata_url(&url);
    let (file_id, server_metadata) = get_metadata_from_server(&client, get_metadata_url).await;


    let post_metadata_url = post_metadata_url(&url);
    let local_metadata = load_file_metadata(pool, file_id).await?;

    println!("metadata blob received from server: {:?}", server_metadata);

    Ok((client, post_metadata_url, local_metadata, server_metadata, file_id))
}

fn get_metadata_url(parent_url: &Url) -> Url {
    let mut endpoint = parent_url.clone();
    endpoint.set_path("/copy/metadata_blob_send");
    endpoint
}

fn post_metadata_url(parent_url: &Url) -> Url {
    let mut endpoint = parent_url.clone();
    endpoint.set_path("/copy/metadata_blob_receive");
    endpoint
}

pub async fn send_metadata_to_server(client: &Client, url: Url, blob: MetadataBlob) {
    client.
        post(url)
        .json(&blob)
        .send()
        .await
        .unwrap();
}

/// Gets the every file and its update time from server
async fn get_metadata_from_server(client: &Client, url: Url) -> (i32, MetadataBlob) {
    client
        .get(url)
        .send()
        .await.unwrap()
        .json()
        .await.unwrap()
}

pub async fn get_new_files_from_server(client: &Client, url: Url) -> Vec<RemoteFile> {
    client
        .get(url)
        .send()
        .await.unwrap()
        .json()
        .await
        .unwrap()
}


/*
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

 */

/*
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

 */

/*
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

 */

