use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("invalid form: expected 10-Q or 10-K")]
    InvalidForm,
    #[error("ticker not found")]
    TickerNotFound,
    #[error("fewer than two filings found for requested form")]
    NotEnoughFilings,
    #[error("SEC fetch or parsing failure: {0}")]
    SecFailure(String),
    #[error("no comparable narrative sections found")]
    NoComparableSections,
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match self {
            AppError::InvalidForm => StatusCode::BAD_REQUEST,
            AppError::TickerNotFound => StatusCode::NOT_FOUND,
            AppError::NotEnoughFilings => StatusCode::NOT_FOUND,
            AppError::SecFailure(_) => StatusCode::BAD_GATEWAY,
            AppError::NoComparableSections => StatusCode::BAD_GATEWAY,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = Json(ErrorResponse {
            error: self.to_string(),
        });

        (status, body).into_response()
    }
}

impl From<reqwest::Error> for AppError {
    fn from(value: reqwest::Error) -> Self {
        AppError::SecFailure(value.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        AppError::SecFailure(value.to_string())
    }
}
