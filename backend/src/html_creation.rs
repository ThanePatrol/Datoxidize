use askama::Template;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use dotenvy::var;

#[derive(Template, Debug)]
#[template(path = "directory.html")]
struct DirEntryHtmlTemplate {
    paths: Vec<String>,
}


fn get_top_level_directories() -> Vec<String> {
    let root_paths = std::fs::read_dir(var("TEST_STORAGE").unwrap())
        .expect(&*format!("Could not find storage_vault at {}", var("TEST_STORAGE").unwrap()));

    root_paths.into_iter().map(|path|
        path.unwrap()
            .path().to_str()
            .unwrap().to_string())
        .collect()
}

pub async fn test_render() -> impl IntoResponse {
    let paths = get_top_level_directories();
    let dir_template = DirEntryHtmlTemplate { paths };
    HtmlTemplate(dir_template)
}

pub struct HtmlTemplate<T>(T);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => axum::response::Html(html).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template. Error: {}", e),
            )
                .into_response(),
        }
    }
}