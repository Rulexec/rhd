pub use thiserror;
pub use serde;

#[derive(thiserror::Error, Debug)]
pub enum RhdError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Custom(String),
}

pub type RhdResult<T> = std::result::Result<T, RhdError>;
