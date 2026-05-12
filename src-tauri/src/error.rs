use serde::Serialize;

/// Structured error type for all Sone backend operations.
/// Serialized as JSON to the frontend via Tauri IPC.
#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum SoneError {
    /// HTTP API returned a non-success status.
    #[error("API error ({status}): {body}")]
    Api { status: u16, body: String },

    /// JSON deserialization or other parse failure.
    #[error("Parse error: {0}")]
    Parse(String),

    /// Network/transport failure (timeout, DNS, connection refused).
    #[error("Network error: {0}")]
    Network(String),

    /// Transient network failure that callers MUST NOT cache.
    /// Includes: connect-fail (DNS / refused), TLS handshake EOF,
    /// timeouts, partial body, HTTP 429, HTTP 5xx. Distinguishing this
    /// from `Network` lets the catalog skip the negative-cache path
    /// when MusicBrainz / Tidal / Wikipedia / Wikidata blip
    /// (D-038 — bug 4: error-swallow caching).
    #[error("Network error (transient): {0}")]
    NetworkTransient(String),

    /// No auth tokens available (user not logged in).
    #[error("Not authenticated")]
    NotAuthenticated,

    /// Client ID / secret not configured.
    #[error("Not configured: {0}")]
    NotConfigured(String),

    /// File system / IO error.
    #[error("IO error: {0}")]
    Io(String),

    /// GStreamer / audio pipeline error.
    #[error("Audio error: {0}")]
    Audio(String),

    /// Encryption / decryption failure.
    #[error("Crypto error: {0}")]
    Crypto(String),

    /// Scrobbling service error.
    #[error("Scrobble error: {0}")]
    Scrobble(String),
}

impl SoneError {
    /// Returns true if this is a network/transport error (either
    /// permanent or transient).
    pub fn is_network(&self) -> bool {
        matches!(
            self,
            SoneError::Network(_) | SoneError::NetworkTransient(_)
        )
    }

    /// D-038 (bug 4): returns true when the error is *transient* and
    /// callers MUST NOT cache the resulting state. Used by
    /// `CatalogService::get_work` / `get_composer` to skip the
    /// 7-day negative cache path when MB/Tidal/Wiki/Wikidata blip.
    pub fn is_transient(&self) -> bool {
        matches!(self, SoneError::NetworkTransient(_))
    }

    /// D-038 — classifier for HTTP status codes constructed manually
    /// by provider call sites. Returns the appropriate variant based
    /// on the status: 429 + 5xx → transient; everything else → permanent.
    pub fn from_http_status(status: u16, message: String) -> Self {
        if status == 429 || (500..600).contains(&status) {
            SoneError::NetworkTransient(message)
        } else {
            SoneError::Network(message)
        }
    }
}

impl From<std::io::Error> for SoneError {
    fn from(e: std::io::Error) -> Self {
        SoneError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for SoneError {
    fn from(e: serde_json::Error) -> Self {
        SoneError::Parse(e.to_string())
    }
}

impl From<reqwest::Error> for SoneError {
    fn from(e: reqwest::Error) -> Self {
        // D-038 — classify reqwest errors into transient vs permanent.
        // Connect failures, timeouts, partial body, decode errors are
        // retryable network blips. Status-coded errors get classified
        // by status (429 + 5xx → transient, others → permanent).
        let msg = e.to_string();
        if e.is_connect()
            || e.is_timeout()
            || e.is_request()
            || e.is_body()
            || e.is_decode()
        {
            return SoneError::NetworkTransient(msg);
        }
        if let Some(status) = e.status() {
            return SoneError::from_http_status(status.as_u16(), msg);
        }
        SoneError::Network(msg)
    }
}

impl From<tauri::Error> for SoneError {
    fn from(e: tauri::Error) -> Self {
        SoneError::Io(e.to_string())
    }
}
