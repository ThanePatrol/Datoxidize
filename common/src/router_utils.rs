use std::path::PathBuf;
use crate::file_utils;

pub async fn show_files() -> String {
    let files = file_utils::get_all_files_from_path(&PathBuf::from("./backend/storage")).unwrap();
    let mut files_as_string = String::new();
    for file in files {
        files_as_string.push_str("| ");
        files_as_string.push_str(file.into_os_string().to_str().unwrap());
        files_as_string.push_str("\n")
    }
    files_as_string
}