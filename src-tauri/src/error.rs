//! Eén fouttype voor de hele backend. De frontend vertaalt fouten via `code` en `key`, zodat
//! meldingen in elke taal in gewone woorden verschijnen. `message` bevat de technische details.

use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Algemene fout met technische details.
    #[error("{0}")]
    Msg(String),
    /// De actie kan alleen met administratorrechten.
    #[error("needs admin: {0}")]
    NeedsAdmin(String),
    /// Een bekende situatie met een vertaalbare sleutel (bv. "verify_failed").
    #[error("{key}")]
    Coded { key: String, params: BTreeMap<String, String> },
    #[error("not found: {0}")]
    NotFound(String),
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Any(#[from] anyhow::Error),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            AppError::NeedsAdmin(_) => "needs_admin",
            AppError::Coded { .. } => "coded",
            AppError::NotFound(_) => "not_found",
            _ => "error",
        }
    }

    pub fn msg(s: impl Into<String>) -> Self {
        AppError::Msg(s.into())
    }

    pub fn coded(key: &str) -> Self {
        AppError::Coded { key: key.to_string(), params: BTreeMap::new() }
    }

    pub fn with(mut self, name: &str, value: impl ToString) -> Self {
        if let AppError::Coded { params, .. } = &mut self {
            params.insert(name.to_string(), value.to_string());
        }
        self
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("AppError", 4)?;
        st.serialize_field("code", self.code())?;
        match self {
            AppError::Coded { key, params } => {
                st.serialize_field("key", key)?;
                st.serialize_field("params", params)?;
            }
            _ => {
                st.serialize_field("key", &Option::<String>::None)?;
                st.serialize_field("params", &BTreeMap::<String, String>::new())?;
            }
        }
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::Msg(s)
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError::Msg(s.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;

/// Vertaalt een "toegang geweigerd" I/O-fout naar een admin-fout.
pub fn map_access_denied(e: std::io::Error, what: &str) -> AppError {
    if e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(5) {
        AppError::NeedsAdmin(what.to_string())
    } else {
        AppError::Io(e)
    }
}
