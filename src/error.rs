use thiserror::Error;
use tower_sessions::session::Error as TowerError;
use askama::Error as AskamaError;
use axum::response::{Html, IntoResponse, Response};
use reqwest::{StatusCode};

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Session persistence error {0}")]
    Session(#[from] TowerError),

    #[error("Template Render Error")]
    Template(#[from] AskamaError),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            _ => self.error_page().into_response(),
        }
    }
}

impl AppError {
    fn error_page(&self) -> impl IntoResponse {
        let (status, msg) = match self {
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Whoops My Bad, Internal Server Error"
            ),
        };
        (status, Html(msg))
    }
}