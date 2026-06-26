use std::collections::HashMap;
use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CredentialsError {
    #[error("failed to read credentials file {path}: {source}")]
    FileRead {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("invalid yaml in credentials file {path}:{line}:{column}: {message}")]
    YamlParse {
        path: PathBuf,
        line: usize,
        column: usize,
        message: String,
    },
}

pub fn load_credentials(path: &Path) -> Result<HashMap<String, String>, CredentialsError> {
    let contents = std::fs::read_to_string(path).map_err(|source| CredentialsError::FileRead {
        path: path.to_path_buf(),
        source,
    })?;

    let credentials: HashMap<String, String> =
        serde_yaml::from_str(&contents).map_err(|e| CredentialsError::YamlParse {
            path: path.to_path_buf(),
            line: e.location().map(|l| l.line()).unwrap_or(0),
            column: e.location().map(|l| l.column()).unwrap_or(0),
            message: e.to_string(),
        })?;

    Ok(credentials)
}
