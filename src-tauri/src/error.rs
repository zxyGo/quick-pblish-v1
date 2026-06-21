use serde::{Serialize, Serializer};

/// 统一错误类型，序列化为 `{ kind, message }` 以匹配前端契约（contracts/*.md 的 AppError）。
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("permission denied: {0}")]
    Permission(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("invalid: {0}")]
    Invalid(String),
}

impl AppError {
    fn kind(&self) -> &'static str {
        match self {
            AppError::NotFound(_) => "NotFound",
            AppError::Permission(_) => "Permission",
            AppError::Io(_) => "Io",
            AppError::Conflict(_) => "Conflict",
            AppError::Invalid(_) => "Invalid",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::NotFound => AppError::NotFound(e.to_string()),
            std::io::ErrorKind::PermissionDenied => AppError::Permission(e.to_string()),
            _ => AppError::Io(e.to_string()),
        }
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Io(format!("sqlite: {e}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Invalid(format!("json: {e}"))
    }
}

impl From<trash::Error> for AppError {
    fn from(e: trash::Error) -> Self {
        AppError::Io(format!("trash: {e}"))
    }
}

pub type AppResult<T> = Result<T, AppError>;
