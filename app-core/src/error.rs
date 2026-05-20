use std::fmt;

#[derive(Debug)]
pub enum NightingaleError {
    Io(std::io::Error),
    Json(serde_json::Error),
    /// A Jellyfin HTTP / parsing failure. `stage` is a short static breadcrumb
    /// (e.g. `"list items"`, `"download cover"`) so the message is consistent
    /// across every call site instead of every adapter rolling its own
    /// `format!("Jellyfin ...: {e}")`.
    Jellyfin {
        stage: &'static str,
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
    /// A Navidrome (Subsonic) HTTP / parsing failure. Same breadcrumb
    /// convention as `Jellyfin`.
    Navidrome {
        stage: &'static str,
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
    Other(String),
}

impl fmt::Display for NightingaleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Json(e) => write!(f, "{e}"),
            Self::Jellyfin { stage, source } => write!(f, "Jellyfin {stage}: {source}"),
            Self::Navidrome { stage, source } => write!(f, "Navidrome {stage}: {source}"),
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for NightingaleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Json(e) => Some(e),
            Self::Jellyfin { source, .. } => Some(source.as_ref()),
            Self::Navidrome { source, .. } => Some(source.as_ref()),
            Self::Other(_) => None,
        }
    }
}

impl NightingaleError {
    pub fn jellyfin<E>(stage: &'static str, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Jellyfin {
            stage,
            source: Box::new(source),
        }
    }

    pub fn navidrome<E>(stage: &'static str, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Navidrome {
            stage,
            source: Box::new(source),
        }
    }
}

impl From<std::io::Error> for NightingaleError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for NightingaleError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

impl From<String> for NightingaleError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

impl From<&str> for NightingaleError {
    fn from(s: &str) -> Self {
        Self::Other(s.to_string())
    }
}
