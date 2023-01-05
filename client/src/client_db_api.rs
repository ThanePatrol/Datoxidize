use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteRow};
use sqlx::{ConnectOptions, Pool, Row, Sqlite, SqlitePool};
use common::file_utils::{FileMetadata, get_all_files_from_path, get_file_metadata_from_path_client, MetadataBlob, VaultMetadata};

pub async fn init_db(db_url: String) -> Result<Pool<Sqlite>, sqlx::Error> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_url.as_str())
        .await?;

    println!("Pool loaded");
    init_metadata_load(&pool).await?;

    Ok(pool)
}

/// Reads file metadata from local files and loads it into the DB on initial client load
/// To do this it needs to get all the vaults and the corresponding paths
/// Then turn it into a nice MetadataBlob struct and add it to the db
async fn init_metadata_load(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    let vault_rows = sqlx::query("select * from client_vaults;")
        .fetch_all(pool)
        .await?;

    let vaults = get_vaults_from_rows(vault_rows);

    for vault in vaults {
        let paths = get_all_files_from_path(&vault.1)
            .expect(&*format!("Could not find paths: {:?}", vault.1));


        let path_with_id = assign_file_ids(pool, paths).await?;


        let file_metadata = get_file_metadata_from_path_client(path_with_id, vault.2, vault.0);

        upsert_database(pool, file_metadata).await?;
    }

    Ok(())
}

/// Does an update/insert on the database, insert files or update them if already exists
/// This is intended for initial DB load
async fn upsert_database(pool: &Pool<Sqlite>, files: Vec<FileMetadata>) -> Result<(), sqlx::Error> {
    for file in files {
        sqlx::query(
            "INSERT OR IGNORE INTO file_metadata (file_id, vault_id, file_path, root_directory, modified_time, file_size)\
                VALUES (?, ?, ?, ?, ?, ?);")
            .bind(file.file_id)
            .bind(file.vault_id)
            .bind(file.full_path.to_str().unwrap().to_string())
            .bind(file.root_directory)
            .bind(file.modified_time)
            .bind(file.file_size)
            .execute(pool)
            .await?;


        sqlx::query("UPDATE file_metadata SET modified_time = ?, file_size = ? WHERE modified_time != ? OR file_size != ?;")
            .bind(file.modified_time)
            .bind(file.file_size)
            .bind(file.modified_time)
            .bind(file.file_size)
            .execute(pool)
            .await?;
    }

    Ok(())
}

/// Queries DB to get the file_id for a specific file_path, if not present, assign as -1
/// This is a helper function as files are organised on the server by file_id but on the client things
/// may end up out of sync, so on initial load the client DB must be updated with metadata to determine
/// what files may need to be synced with the server
async fn assign_file_ids(pool: &Pool<Sqlite>, paths: Vec<PathBuf>) -> Result<Vec<(i32, PathBuf)>, sqlx::Error> {
    let mut tuples = Vec::new();
    for path in paths {
        let row = sqlx::query("select file_id from file_metadata where file_path == ?")
            .bind(path.to_str().unwrap())
            .fetch_one(pool)
            .await;

        let file_id = match row {
            Ok(r) => r.get::<i32, _>(0),
            Err(_) => -1
        };

        tuples.push((file_id, path))
    }
    Ok(tuples)
}

/// Takes a vector of rows from client_db vault. Returns a vector of tuples
/// 0th index is the vault_id, 1st is the absolute path of the vault
fn get_vaults_from_rows(rows: Vec<SqliteRow>) -> Vec<(i32, PathBuf, String)> {
    rows
        .iter()
        .map(|row| {
            let id = row.get::<i32, _>(0);
            let path = PathBuf::from(row.get::<String, _>(1));
            let root_dir = row.get::<String, _>(2);
            (id, path, root_dir)
        })
        .collect::<Vec<(i32, PathBuf, String)>>()
}

/// Loads metadata from DB into a MetadataBlob struct to send to the server
pub async fn load_file_metadata(pool: &Pool<Sqlite>) -> Result<MetadataBlob, sqlx::Error> {
    let vaults = get_all_vaults(pool).await?;
    let mut blob = MetadataBlob {
        vaults: HashMap::new(),
    };

    for vault in vaults {
        let rows = sqlx::query("select file_id, file_path, root_directory, modified_time, file_size from file_metadata where vault_id == ?;")
            .bind(vault)
            .fetch_all(pool)
            .await?;

        let files = build_file_metadata_for_vault(vault, rows);

        let vault_metadata = VaultMetadata {
            files,
            vault_id: vault,
        };

        blob.vaults.insert(vault, vault_metadata);
    }

    Ok(blob)
}

async fn get_all_vaults(pool: &Pool<Sqlite>) -> Result<Vec<i32>, sqlx::Error> {
    let rows = sqlx::query("select vault_id from client_vaults;")
        .fetch_all(pool)
        .await?;

    let vaults = rows
        .iter()
        .map(|row| row.get::<i32, _>(0))
        .collect::<Vec<i32>>();
    Ok(vaults)
}

fn build_file_metadata_for_vault(vault: i32, rows: Vec<SqliteRow>) -> Vec<FileMetadata> {
    let mut files = Vec::new();

    rows
        .iter()
        .for_each(|row| {
            let path = PathBuf::from(row.get::<String, _>(1));
            let file = FileMetadata {
                full_path: path,
                root_directory: row.get::<String, _>(2),
                modified_time: row.get::<i64, _>(3),
                file_size: row.get::<i64, _>(4),
                vault_id: vault,
                file_id: row.get::<i32, _>(0),
            };
            files.push(file);
        });
    files
}

