use crate::client_db_api::load_file_metadata;
use common::file_utils::{MetadataBlob};
use common::RemoteFile;
use common::{common_db_utils, file_utils};
use reqwest::{Client, Url};
use sqlx::{Pool, Sqlite};
use common::common_db_utils::{read_file_contents_from_disk_and_metadata, upsert_database};

/// Main api that is called on launch of client
/// Will make request to server for a list of all files and their metadata
/// Once received, go through the list of files, if there is something more recent on server
/// It makes a request for that file, if the file is more recent on the client, send it to server
pub async fn init_metadata_sync(url: Url, pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    let client = Client::new();

    // Gets metadata from server via http
    let (file_id, server_metadata) = get_metadata_from_server(&client, &url).await;

    // Gets local metadata from DB - Also updates file id's to newest based upon the latest_file_id
    // received from server
    let local_metadata = load_file_metadata(pool, file_id).await?;
    println!("local metadata: {:?}", local_metadata);

    // Gets metadata diff and sends it to server which is then inserted into db
    let metadata_diff = file_utils::get_metadata_diff(local_metadata, server_metadata);

    let (new_for_client, new_for_server) = metadata_diff.destruct_into_tuple();
    println!("new for client: {:?}", new_for_client);
    println!("new for server: {:?}", new_for_server);

    //upsert_database(pool, new_for_client.clone().convert_to_metadata_vec()).await?;

    post_metadata_diff_to_server(&client, &url, &new_for_server).await;

    //todo stop some of this metadata sending
    //general structure should be:
    //1. read files on disk for client and server [✅]
    //2. client and server update their dbs [✅]
    //3. client requests metadata from server [✅]
    //4. client compares with their own local metadata [✅]
    //5. client inserts the servers metadata [✅]
    //6. client sends metadata to server
    //7. server inserts the clients metadata
    //8. client requests for files it needs to be updated and saves to disk - updating the metadata to what is stored in db
    //9. client determines what files to send
    //10. client reads files from disk and sends to server - server saves files - updating the metadata to what is stored in db
    //11. client updates db
    //12. client sends message server, indicating to update db
    //13. init sync is done



    // requests for files from server to update and/or add, also upsert database
    let files = get_new_files_for_client(&client, &url, &new_for_client).await;
    let vault_and_root_paths = common_db_utils::get_vault_id_and_root_directories(pool).await?;
    //common_db_utils::upsert_database(pool, new_for_client.convert_to_metadata_vec()).await?;
    file_utils::save_remote_files_to_disk(files, vault_and_root_paths);

    //todo - read files into vec<remotefile> and send to the server
    let local_files= read_file_contents_from_disk_and_metadata(
        pool,
        &new_for_server.convert_to_metadata_vec())
        .await;
    send_files_to_server(&client, &url, local_files)
        .await;

    Ok(())
}


/// Gets the every file and its update time from server
async fn get_metadata_from_server(client: &Client, parent_url: &Url) -> (i32, MetadataBlob) {
    fn create_get_metadata_url(parent_url: &Url) -> Url {
        let mut endpoint = parent_url.clone();
        endpoint.set_path("/copy/metadata_blob_send");
        endpoint
    }

    let get_metadata_url = create_get_metadata_url(parent_url);

    client.get(get_metadata_url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn post_metadata_diff_to_server(client: &Client, parent_url: &Url, diff: &MetadataBlob) {
    fn create_post_metadata_diff_url(parent_url: &Url) -> Url {
        let mut endpoint = parent_url.clone();
        endpoint.set_path("/copy/metadata_diff_receive");
        endpoint
    }

    let metadata_diff_url = create_post_metadata_diff_url(parent_url);

    client.post(metadata_diff_url)
        .json(&diff)
        .send()
        .await
        .unwrap();
}

/// Part of init sync for server and client:
/// Takes the Client and a MetadataBlob consisting of files that are needed for the client
/// POST to server with a body of a list of files needed by the client
/// GET the files in the list
async fn get_new_files_for_client(
    client: &Client,
    parent_url: &Url,
    blob: &MetadataBlob,
) -> Vec<RemoteFile> {
    fn create_post_required_files_url(parent_url: &Url) -> Url {
        let mut endpoint = parent_url.clone();
        endpoint.set_path("/copy/client_needs");
        endpoint
    }

    fn create_get_files_init_url(parent_url: &Url) -> Url {
        let mut endpoint = parent_url.clone();
        endpoint.set_path("/copy/send_files_to_client_from_state");
        endpoint
    }

    let update_state_url = create_post_required_files_url(parent_url);

    //sends a message to the server, updating the state with the list of files required
    client
        .post(update_state_url)
        .json(&blob)
        .send()
        .await
        .unwrap();

    //requests for the files from the server (files not present on client)
    let get_files_url = create_get_files_init_url(parent_url);
    client
        .get(get_files_url)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn send_files_to_server(client: &Client, parent_url: &Url, files: Vec<RemoteFile>) {
    fn create_url_to_send_files_to_server(parent_url: &Url) -> Url {
        let mut endpoint = parent_url.clone();
        endpoint.set_path("/copy/receive_files_from_client");
        endpoint
    }

    let url = create_url_to_send_files_to_server(parent_url);
    client.post(url)
        .json(&files)
        .send()
        .await
        .unwrap();
}





