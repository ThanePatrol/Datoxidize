use crate::client_db_api::load_file_metadata;
use axum::http;
use common::config_utils::{DirectoryConfig, VaultConfig};
use common::file_utils;
use common::file_utils::{FileMetadata, MetadataBlob, MetadataDiff};
use common::RemoteFile;
use http::StatusCode;
use reqwest::{Client, Url};
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Barrier;

/// Main api that is called on launch of client
/// Will make request to server for a list of all files and their metadata
/// Once received, go through the list of files, if there is something more recent on server
/// It makes a request for that file, if the file is more recent on the client, send it to server
pub async fn init_metadata_sync(
    url: Url,
    pool: &Pool<Sqlite>,
) -> Result<(Client, Url, Vec<RemoteFile>, i32), sqlx::Error> {
    let client = Client::new();

    /// Gets metadata from server via http
    let get_metadata_url = get_metadata_url(&url);
    let (file_id, server_metadata) = get_metadata_from_server(&client, get_metadata_url).await;

    /// Gets local metadata from DB - Also updates file id's to newest
    let post_metadata_url = post_metadata_url(&url);
    let local_metadata = load_file_metadata(pool, file_id).await?;

    /// Gets metadata diff and sends it to server which is then inserted into db
    let metadata_diff = file_utils::get_metadata_diff(local_metadata, server_metadata);
    let (client_new, server_new) = metadata_diff.destruct_into_tuple();
    let metadata_diff_url = post_metadata_diff_url(&url);
    post_metadata_diff_to_server(&client, metadata_diff_url, server_new).await;

    // requests for files from server to update and/or add
    let files = get_new_files_for_client(&client, &url, client_new).await;

    //todo - send files required for server to the server

    println!("Files received from server: {:?}", files);

    Ok((
        client,
        post_metadata_url,
        files,
        file_id,
    ))
}



pub async fn send_metadata_to_server(client: &Client, url: Url, blob: MetadataBlob) {
    client.post(url).json(&blob).send().await.unwrap();
}

/// Gets the every file and its update time from server
async fn get_metadata_from_server(client: &Client, url: Url) -> (i32, MetadataBlob) {
    client.get(url).send().await.unwrap().json().await.unwrap()
}

pub async fn get_new_files_from_server(client: &Client, url: Url) -> Vec<RemoteFile> {
    client.get(url).send().await.unwrap().json().await.unwrap()
}

async fn post_metadata_diff_to_server(client: &Client, url: Url, diff: MetadataBlob) {
    client.post(url).json(&diff).send().await.unwrap();
}

/// Part of init sync for server and client:
/// Takes the Client and a MetadataBlob consisting of files that are needed for the client
/// POST to server with a body of a list of files needed by the client
/// GET the files in the list
async fn get_new_files_for_client(client: &Client, parent_url: &Url, blob: MetadataBlob) -> Vec<RemoteFile> {
    let update_state_url = post_required_files_url(parent_url);
    println!("here");

    client
        .post(update_state_url)
        .json(&blob)
        .send()
        .await
        .unwrap();

    let get_files_url = get_files_init_url(parent_url);
    client
        .get(get_files_url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
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

fn post_metadata_diff_url(parent_url: &Url) -> Url {
    let mut endpoint = parent_url.clone();
    endpoint.set_path("/copy/metadata_diff_receive");
    endpoint
}

fn post_required_files_url(parent_url: &Url) -> Url {
    let mut endpoint = parent_url.clone();
    endpoint.set_path("/copy/client_needs");
    endpoint
}

fn get_files_init_url(parent_url: &Url) -> Url {
    let mut endpoint = parent_url.clone();
    endpoint.set_path("/copy/send_files_to_client_from_state");
    endpoint
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
