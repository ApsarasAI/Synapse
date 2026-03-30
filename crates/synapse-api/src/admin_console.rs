use axum::response::Html;

const ADMIN_CONSOLE_HTML: &str = include_str!("admin_console.html");

pub fn page() -> Html<&'static str> {
    Html(ADMIN_CONSOLE_HTML)
}
