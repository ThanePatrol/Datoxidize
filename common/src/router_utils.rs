
pub async fn show_files() -> String {
    let files = std::fs::read_dir("./storage").unwrap();
    let mut files_as_string = String::new();
    for file in files {
        files_as_string.push_str("| ");
        files_as_string.push_str(file.unwrap().file_name().to_str().unwrap());
        files_as_string.push_str("\n")
    }
    files_as_string
}