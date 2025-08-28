use thiserror::Error;
use tower_sessions::session::Error as TowerError;
use sqlx::Error as SqlxError;
use askama::Error as AskamaError;
use axum::response::{Html, IntoResponse, Response};
use reqwest::{StatusCode};
use reqwest::Error as ReqwestError;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Session persistence error {0}")]
    Session(#[from] TowerError),

    #[error("Template Render Error {0}")]
    Template(#[from] AskamaError),

    #[error("Database Error {0}")]
    Database(#[from] SqlxError),

    #[error("ZAuth authentication error: {0}")]
    Zauth(String),

    #[error("HTTP request error {0}")]
    Reqwest(#[from] ReqwestError),
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