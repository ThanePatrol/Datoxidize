use crate::file_utils::{FileMetadata, MetadataBlob};
use crate::{file_utils, RemoteFile};
use sqlx::sqlite::SqliteRow;
use sqlx::{Pool, Row, Sqlite};
use std::path::PathBuf;

/// Reads data from the file_system and updates the Database accordingly
/// Ensures that files that have changed on disk while syncing is not active are
/// added to the db so they can be synced on initial load
/// NB - All functions in this file assume that the client and database table names are identical
/// could be separated by feeding in the queries but simpler to have a tighter dependence
/// Assumes table called vaults
#[tokio::main]
pub async fn init_metadata_load_into_db(
    pool: &Pool<Sqlite>,
    is_server: bool,
) -> Result<(), sqlx::Error> {
    let vault_rows = sqlx::query("select * from vaults;").fetch_all(pool).await?;

    let vaults = get_vaults_from_rows(vault_rows);

    for (vault_id, vault_path, root_dir) in vaults {
        let paths = file_utils::get_all_files_from_path(&vault_path)
            .expect(&*format!("Could not find paths: {:?}", vault_path));

        println!("paths in init_metadata_load_into_db: {:?}", paths);

        remove_old_entries_from_db(pool).await?;

        let path_with_id = assign_file_ids(pool, paths, is_server).await?;

        let file_metadata =
            file_utils::get_file_metadata_from_path(path_with_id, root_dir, vault_path, vault_id);

        //todo - use https://stackoverflow.com/questions/44419890/replacing-path-parts-in-rust
        // to create absolute paths on both the server and the client
        // server could potentially have a relative path but client should have absolute
        // make sure to use https://doc.rust-lang.org/std/path/constant.MAIN_SEPARATOR.html
        // instead of /
        // strip prefix using .strip_prefix(), take the absolute path parent, add separator, add vault dir, add separator
        // then add the rest of the content that was stripped

        upsert_database(pool, file_metadata).await?;
    }
    Ok(())
}

/// Does an update/insert on the database, insert files or update them if already exists
/// This is intended for initial DB load
/// sets modified_time and file_size to the current file
pub async fn upsert_database(
    pool: &Pool<Sqlite>,
    files: Vec<FileMetadata>,
) -> Result<(), sqlx::Error> {
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

        sqlx::query("UPDATE file_metadata SET modified_time = ?, file_size = ? WHERE file_id == ? AND (modified_time != ? OR file_size != ?);")
            .bind(file.modified_time)
            .bind(file.file_size)
            .bind(file.file_id)
            .bind(file.modified_time)
            .bind(file.file_size)
            .execute(pool)
            .await?;
    }
    Ok(())
}

/// Reads through all the paths given from the Database, if not present then remove entry from database
async fn remove_old_entries_from_db(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    async fn get_paths_from_db(pool: &Pool<Sqlite>) -> Result<Vec<PathBuf>, sqlx::Error> {
        let rows = sqlx::query("select file_path from file_metadata;")
            .fetch_all(pool)
            .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let path = row.get::<String, _>(0);
                PathBuf::from(path)
            })
            .collect::<Vec<PathBuf>>())
    }

    let db_paths = get_paths_from_db(pool).await?;

    for path in db_paths {
        println!("checking path: {:?}", path);
        if path.exists() {
            continue;
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
/// 0th index is the vault_id, 1st is the absolute path of the vault, 2nd is the root directory as String

fn get_vaults_from_rows(rows: Vec<SqliteRow>) -> Vec<(i32, PathBuf, String)> {
    rows.iter()
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
async fn assign_file_ids(
    pool: &Pool<Sqlite>,
    paths: Vec<PathBuf>,
    is_server: bool,
) -> Result<Vec<(i32, PathBuf)>, sqlx::Error> {
    let mut paths_with_ids = Vec::new();

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

        paths_with_ids.push((file_id, path))
    }

    Ok(paths_with_ids)
}

async fn get_next_id(pool: &Pool<Sqlite>) -> Result<i32, sqlx::Error> {
    let query = sqlx::query("select MAX(file_id) from file_metadata;")
        .fetch_one(pool)
        .await?;
    let next_id = query.get::<i32, _>(0) + 1;
    Ok(next_id)
}

/// Used for getting the actual file contents from metadata
/// File path is stored in the database, hence the pool
pub async fn get_file_contents_from_metadata(
    pool: &Pool<Sqlite>,
    metadata: &Vec<FileMetadata>,
) -> Vec<RemoteFile> {
    let mut files = Vec::with_capacity(metadata.len());
    for data in metadata {
        let path = get_file_paths_from_id(pool, &data)
            .await
            .expect(&*format!("Error reading {:?}", data));

        let file = RemoteFile::new(
            path,
            data.root_directory.clone(),
            data.vault_id,
            data.file_id,
        );
        files.push(file);
    }
    files
}

/// Takes a vector of metadata, gets the path from the DB
/// Assumes that all file_ids are already added to the db
async fn get_file_paths_from_id(
    pool: &Pool<Sqlite>,
    file: &FileMetadata,
) -> Result<PathBuf, sqlx::Error> {
    let str_path = sqlx::query("select file_path from file_metadata where file_id == ?")
        .bind(file.file_id)
        .fetch_one(pool)
        .await?;
    let path = str_path.get::<String, _>(0);
    Ok(PathBuf::from(path))
}

/// Gets the root directory for all vaults of the server
/// Server stores data as:
/// /home/root/storage/vault0
/// /home/root/storage/vault1
/// The function will return a vec of pathbufs eg: ["/home/root/storage/vault0", "/home/root/storage/vault1"]
async fn get_root_local_root_directory(pool: &Pool<Sqlite>) -> Result<Vec<(i32, PathBuf)>, sqlx::Error> {
    let root_paths = sqlx::query("select vault_id, abs_path from vaults;")
        .fetch_all(pool)
        .await?;

    let root_paths = root_paths
        .iter()
        .map(|row| {

            let raw_string = row.get::<String, _>(1);

            (row.get::<i32, _>(0), PathBuf::from(raw_string))
        })
        .collect::<Vec<(i32, PathBuf)>>();
    Ok(root_paths)
}

/// Iterates through a metadata blob - finds matching vaults then updates all the paths from the metadatablob
/// to the correct path for the server using file_utils
pub async fn convert_root_dirs_of_metadata(pool: &Pool<Sqlite>, metadata: &mut MetadataBlob) -> Result<(), sqlx::Error> {
    let root_dirs = get_root_local_root_directory(pool).await?;

    for (id, metadata) in metadata.vaults.iter_mut() {
        for (vault_id, root_path) in root_dirs.iter() {
            if vault_id == id {
                for file in metadata.files.iter_mut() {
                    let new_path = file_utils::convert_path_to_local(&file.full_path, &file.absolute_root_dir, root_path);
                    file.full_path = new_path;
                }
            }
        }
    }

    Ok(())
}
