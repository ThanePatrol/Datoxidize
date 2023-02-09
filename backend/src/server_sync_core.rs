use crate::ApiState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use common::file_utils::{FileMetadata, MetadataBlob, ServerPresent};
use common::{common_db_utils, file_utils, RemoteFile};
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn save_user_required_files(
    State(state): State<Arc<Mutex<ApiState>>>,
    Json(payload): Json<MetadataBlob>,
) -> impl IntoResponse {
    println!("client request: {:?}", payload);
    let payload_on_server= payload
        .convert_to_metadata_vec()
        .into_iter()
        .filter(|files| files.present_on_server == ServerPresent::Yes)
        .collect::<Vec<FileMetadata>>();

    state.lock().await.client_requested = payload_on_server;
    StatusCode::OK
}

pub async fn get_remote_files_for_client(
    State(state): State<Arc<Mutex<ApiState>>>,
) -> impl IntoResponse {
    let state = &state.lock().await;
    let files =
        common_db_utils::read_file_contents_from_disk_and_metadata(&state.pool, &state.client_requested)
            .await;
    Json(files)
}

pub async fn receive_files_from_client(
    State(state): State<Arc<Mutex<ApiState>>>,
    Json(payload): Json<Vec<RemoteFile>>
) -> impl IntoResponse {
    let state = &state.lock().await;
    let vault_and_root_paths = common_db_utils::get_vault_id_and_root_directories(&state.pool)
        .await
        .expect(&*format!("Error reading from database with {:?}", payload));
    file_utils::save_remote_files_to_disk(payload, vault_and_root_paths);
    StatusCode::OK
}

/*-----------------------------OLD STUFF BELOW-----------------------------------------*/

/*



static VAULT_CONFIGS: Lazy<HashMap<i32, VaultConfig>> = Lazy::new(|| {
        deserialize_vault_config()
});

//pub async fn send_list_of_newer_files_to_client(local_files: Vec<RemoteFile>, )

/// Returns true if sender has a more recent copy of a file than local
pub fn is_client_more_recent_than_server(remote: &RemoteFile, local: &PathBuf) -> bool {
    if !Path::new(local).exists() {
        return true
    }
    let mdata_err_msg = "Can't read metadata on this platform";

    let server_metadata = fs::metadata(local)
        .expect(&*format!("Error reading metadata from {:?}", local));

    //remote.metadata.1;
    let server_time = server_metadata.modified()
        .expect(mdata_err_msg);

    UNIX_EPOCH > server_time
}

//todo - set metadata from local to payload
//todo log error messages rather than unwrapping
pub async fn sync_file_with_server(payload: RemoteFile) -> bool {
    let vaults = &VAULT_CONFIGS;
    let config = vaults.get(&payload.vault_id)
        .expect("Vault_id not found");
    let server_file_path = get_server_path(&payload, &config);
    println!("Path: {}", server_file_path.display());

    if is_client_more_recent_than_server(&payload, &server_file_path) {
        //copy_file_to_server(&payload, &config).await.unwrap();
        true
    } else {
        false
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use common::config_utils::deserialize_vault_config;
    use std::time::SystemTime;

    #[test]
    fn test_get_server_path() {
        let file = RemoteFile::new_empty(
            PathBuf::from("../datoxidize/example_dir/lophostemon_occurrences.csv"),
        "example_dir".to_string(),
        0,
        );

        let vault = deserialize_vault_config();
        let path = get_server_path(&file, &vault.get(&0).unwrap());
        assert_eq!(path.as_os_str().to_str().unwrap(), "./storage/vault0/lophostemon_occurrences.csv");
    }

    #[test]
    fn test_get_nested_server_path() {
        let client = RemoteFile {
            full_path: PathBuf::from("../datoxidize/example_dir/serve_test/another/test.txt"),
            root_directory: "example_dir".to_string(),
            contents: vec![],
            metadata: (SystemTime::now(), SystemTime::now(), 0),
            vault_id: 0,
        };
        let vault = deserialize_vault_config();
        let path = get_server_path(&client, &vault.get(&0).unwrap());
        assert_eq!(path.as_os_str().to_str().unwrap(), "./storage/vault0/serve_test/another/test.txt")
    }

    #[test]
    fn is_client_more_recent_than_server() {

    }

*/
