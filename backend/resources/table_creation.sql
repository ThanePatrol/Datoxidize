CREATE TABLE file_metadata (
    file_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    vault_id INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    root_directory TEXT NOT NULL,
    access_time INTEGER NOT NULL,
    modified_time INTEGER NOT NULL,
    file_size INTEGER NOT NULL
);
