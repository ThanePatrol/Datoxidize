use std::path::PathBuf;
use sqlx::{Pool, Row, Sqlite};
use sqlx::sqlite::SqliteRow;
use crate::file_utils;
use crate::file_utils::FileMetadata;

/// Reads data from the file_system and updates the Database accordingly
/// Ensures that files that have changed on disk while syncing is not active are
/// added to the db so they can be synced on initial load
/// NB - All functions in this file assume that the client and database table names are identical
/// could be separated by feeding in the queries but simpler to have a tighter dependence
#[tokio::main]
pub async fn init_metadata_load_into_db(pool: &Pool<Sqlite>, is_server: bool) -> Result<(), sqlx::Error>{
    let vault_rows = sqlx::query("select * from vaults;")
        .fetch_all(pool)
        .await?;

    let vaults = get_vaults_from_rows(vault_rows);

    for vault in vaults {
        let paths = file_utils::get_all_files_from_path(&vault.1)
            .expect(&*format!("Could not find paths: {:?}", vault.1));
        println!("paths in init_metadata_load_into_db: {:?}", paths);

        remove_old_entries_from_db(pool, &paths).await?;

        let path_with_id = assign_file_ids(pool, paths, is_server).await?;

        let file_metadata = file_utils::get_file_metadata_from_path(path_with_id, vault.2, vault.0);

        upsert_database(pool, file_metadata).await?;
    }
    Ok(())
}


/// Does an update/insert on the database, insert files or update them if already exists
/// This is intended for initial DB load
pub async fn upsert_database(pool: &Pool<Sqlite>, files: Vec<FileMetadata>) -> Result<(), sqlx::Error> {
    for file in files {
        println!("executing upsert for: {:?}", file);

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

async fn remove_old_entries_from_db(pool: &Pool<Sqlite>, paths: &Vec<PathBuf>) -> Result<(), sqlx::Error> {
    for path in paths {
        if path.exists() {
            continue
        }
        println!("deleting {:?}", path);
        sqlx::query("delete from file_metadata where file_path == ?")
            .bind(path.to_str().unwrap())
            .execute(pool)
            .await?;
    }
    Ok(())
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


/// Queries DB to get the file_id for a specific file_path, if not present, assign as -1
/// This is a helper function as files are organised on the server by file_id but on the client things
/// may end up out of sync, so on initial load the client DB must be updated with metadata to determine
/// what files may need to be synced with the server
async fn assign_file_ids(pool: &Pool<Sqlite>, paths: Vec<PathBuf>, is_server: bool) -> Result<Vec<(i32, PathBuf)>, sqlx::Error> {
    let mut tuples = Vec::new();
    for path in paths {
        let row = sqlx::query("select file_id from file_metadata where file_path == ?")
            .bind(path.to_str().unwrap())
            .fetch_one(pool)
            .await;

        let file_id;
        if is_server {
            file_id = match row {
                Ok(r) => r.get::<i32, _>(0),
                Err(_) => get_next_id(pool).await?,
            }
        } else {
            file_id = match row {
                Ok(r) => r.get::<i32, _>(0),
                Err(_) => -1,
            };
        }

        tuples.push((file_id, path))
    }
    Ok(tuples)
}

async fn get_next_id(pool: &Pool<Sqlite>) -> Result<i32, sqlx::Error> {
    let query = sqlx::query("select MAX(file_id) from file_metadata;")
        .fetch_one(pool)
        .await?;
    let next_id = query.get::<i32, _>(0) + 1;
    Ok(next_id)
}