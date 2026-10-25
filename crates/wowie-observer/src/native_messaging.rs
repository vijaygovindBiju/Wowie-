use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use url::Url;

/// Maximum allowable Native Messaging payload (1 MiB) per standard.
pub const MAX_MESSAGE_SIZE: usize = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum NativeMessagingError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Message exceeded maximum allowed size of {0} bytes")]
    MessageTooLarge(usize),
    #[error("Unexpected end of file while reading message")]
    UnexpectedEof,
    #[error("Invalid JSON payload: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("Invalid or non-http(s) origin: {0}")]
    InvalidOrigin(String),
}

/// Message payload emitted by the Wowie Focus Companion browser extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserMessage {
    /// Active tab origin (e.g. "https://docs.rs").
    pub origin: String,
    /// Whether the browser window containing this tab is active.
    #[serde(default = "default_true")]
    pub is_active_window: bool,
    /// Internal tab ID if provided by the browser.
    #[serde(default)]
    pub tab_id: Option<u64>,
    /// Optional sanitized title of the tab.
    #[serde(default)]
    pub title: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Response returned to the browser extension by the native messaging host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acknowledged_origin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Shared cache entry read by desktop activity observers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedOriginRecord {
    pub origin: String,
    pub timestamp_epoch_sec: u64,
    pub is_active_window: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Reads a length-prefixed native messaging frame from a reader.
/// Returns `Ok(None)` on clean EOF before any bytes are read.
pub fn read_native_message<R: Read>(
    reader: &mut R,
) -> Result<Option<Vec<u8>>, NativeMessagingError> {
    let mut len_bytes = [0u8; 4];
    let mut total_read = 0;
    while total_read < 4 {
        match reader.read(&mut len_bytes[total_read..]) {
            Ok(0) => {
                if total_read == 0 {
                    return Ok(None);
                }
                return Err(NativeMessagingError::UnexpectedEof);
            }
            Ok(n) => {
                total_read += n;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(NativeMessagingError::Io(e)),
        }
    }

    let length = u32::from_ne_bytes(len_bytes) as usize;
    if length > MAX_MESSAGE_SIZE {
        return Err(NativeMessagingError::MessageTooLarge(length));
    }

    let mut buffer = vec![0u8; length];
    reader
        .read_exact(&mut buffer)
        .map_err(|_| NativeMessagingError::UnexpectedEof)?;

    Ok(Some(buffer))
}

/// Writes a length-prefixed native messaging frame to a writer.
pub fn write_native_message<W: Write>(
    writer: &mut W,
    payload: &[u8],
) -> Result<(), NativeMessagingError> {
    if payload.len() > MAX_MESSAGE_SIZE {
        return Err(NativeMessagingError::MessageTooLarge(payload.len()));
    }

    let len_bytes = (payload.len() as u32).to_ne_bytes();
    writer.write_all(&len_bytes)?;
    writer.write_all(payload)?;
    writer.flush()?;
    Ok(())
}

/// Strips paths, search queries, auth credentials, and fragments from a URL or raw origin,
/// returning purely the normalized origin (e.g. `https://example.com` or `http://localhost:8080`).
///
/// Rejects non-HTTP(S) schemes (e.g. `chrome:`, `javascript:`, `file:`, `about:`).
pub fn sanitize_origin(raw: &str) -> Result<String, NativeMessagingError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(NativeMessagingError::InvalidOrigin(
            "Empty origin string".to_string(),
        ));
    }

    let parsed = Url::parse(trimmed).map_err(|e| {
        NativeMessagingError::InvalidOrigin(format!("URL parse failure ({e}): {trimmed}"))
    })?;

    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(NativeMessagingError::InvalidOrigin(format!(
            "Forbidden scheme '{scheme}' (only http and https are permitted)"
        )));
    }

    // `url.origin().ascii_serialization()` formats purely scheme://host(:port)
    let origin_str = parsed.origin().ascii_serialization();
    if origin_str == "null" || origin_str.is_empty() {
        return Err(NativeMessagingError::InvalidOrigin(format!(
            "Opaque or null origin extracted from {trimmed}"
        )));
    }

    Ok(origin_str)
}

/// Returns the platform-specific default runtime directory for Wowie IPC / origin caches.
pub fn default_origin_cache_dir() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("wowie")
    } else {
        std::env::temp_dir().join(format!("wowie-{}", unsafe_user_id()))
    }
}

/// Helper to get current UID or fallback to 1000 without requiring libc.
fn unsafe_user_id() -> u32 {
    #[cfg(unix)]
    {
        // Safe read of UID via standard std::process environment or /proc if possible
        if let Ok(uid_str) = std::env::var("UID") {
            if let Ok(uid) = uid_str.parse::<u32>() {
                return uid;
            }
        }
    }
    1000
}

/// Returns the default path for `active_browser_origin.json`.
pub fn default_origin_cache_path() -> PathBuf {
    default_origin_cache_dir().join("active_browser_origin.json")
}

/// Atomically persists the active origin into the cache file.
pub fn write_cached_origin(
    path: &Path,
    origin: &str,
    is_active_window: bool,
    title: Option<&str>,
) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let now_sec = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());

    let record = CachedOriginRecord {
        origin: origin.to_string(),
        timestamp_epoch_sec: now_sec,
        is_active_window,
        title: title.map(|t| t.to_string()),
    };

    let serialized = serde_json::to_vec_pretty(&record)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // Atomic write via temporary file
    let tmp_path = path.with_extension("tmp");
    {
        let mut file = File::create(&tmp_path)?;
        file.write_all(&serialized)?;
        file.flush()?;
    }
    fs::rename(&tmp_path, path)?;

    Ok(())
}

/// Reads the cached origin if it was updated within `max_age` and marks the window as active.
pub fn read_cached_origin(path: &Path, max_age: Duration) -> Option<String> {
    if !path.exists() {
        return None;
    }

    let contents = fs::read(path).ok()?;
    let record: CachedOriginRecord = serde_json::from_slice(&contents).ok()?;

    if !record.is_active_window {
        return None;
    }

    let now_sec = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());

    let age_sec = now_sec.saturating_sub(record.timestamp_epoch_sec);
    if age_sec > max_age.as_secs() {
        return None;
    }

    Some(record.origin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_native_messaging_roundtrip() {
        let msg = BrowserMessage {
            origin: "https://docs.rs".to_string(),
            is_active_window: true,
            tab_id: Some(101),
            title: Some("Standard Library - Rust".to_string()),
        };
        let json_bytes = serde_json::to_vec(&msg).unwrap();

        let mut stream = Vec::new();
        write_native_message(&mut stream, &json_bytes).unwrap();

        assert_eq!(stream.len(), 4 + json_bytes.len());

        let mut cursor = Cursor::new(stream);
        let read_bytes = read_native_message(&mut cursor).unwrap().unwrap();
        assert_eq!(read_bytes, json_bytes);

        let parsed: BrowserMessage = serde_json::from_slice(&read_bytes).unwrap();
        assert_eq!(parsed, msg);
    }

    #[test]
    fn test_native_messaging_clean_eof() {
        let mut cursor = Cursor::new(Vec::<u8>::new());
        let result = read_native_message(&mut cursor).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_native_messaging_partial_length_unexpected_eof() {
        let partial_bytes = vec![1, 2];
        let mut cursor = Cursor::new(partial_bytes);
        let err = read_native_message(&mut cursor);
        assert!(matches!(err, Err(NativeMessagingError::UnexpectedEof)));
    }

    #[test]
    fn test_native_messaging_oversized_payload_rejection() {
        let huge_len = (MAX_MESSAGE_SIZE + 1) as u32;
        let len_bytes = huge_len.to_ne_bytes();
        let mut cursor = Cursor::new(len_bytes);
        let err = read_native_message(&mut cursor);
        assert!(matches!(
            err,
            Err(NativeMessagingError::MessageTooLarge(len)) if len == MAX_MESSAGE_SIZE + 1
        ));
    }

    #[test]
    fn test_sanitize_origin_strips_path_query_and_fragment() {
        let raw = "https://github.com/rust-lang/rust/pull/12345?query=focus#comment-42";
        let sanitized = sanitize_origin(raw).unwrap();
        assert_eq!(sanitized, "https://github.com");

        let with_port = "http://localhost:8080/dashboard?admin=true";
        assert_eq!(sanitize_origin(with_port).unwrap(), "http://localhost:8080");
    }

    #[test]
    fn test_sanitize_origin_rejects_dangerous_schemes() {
        assert!(sanitize_origin("chrome://extensions").is_err());
        assert!(sanitize_origin("file:///etc/hosts").is_err());
        assert!(sanitize_origin("javascript:alert(1)").is_err());
        assert!(sanitize_origin("about:blank").is_err());
        assert!(sanitize_origin("data:text/html,hello").is_err());
        assert!(sanitize_origin("").is_err());
    }

    #[test]
    fn test_origin_cache_write_and_read_freshness() {
        let temp_dir = std::env::temp_dir().join(format!("wowie_test_{}", uuid::Uuid::new_v4()));
        let cache_path = temp_dir.join("test_origin.json");

        // Write active origin
        write_cached_origin(&cache_path, "https://crates.io", true, Some("crates.io")).unwrap();

        // Read immediately (fresh)
        let loaded = read_cached_origin(&cache_path, Duration::from_secs(10));
        assert_eq!(loaded, Some("https://crates.io".to_string()));

        // Inactive window returns None
        write_cached_origin(&cache_path, "https://crates.io", false, None).unwrap();
        assert_eq!(
            read_cached_origin(&cache_path, Duration::from_secs(10)),
            None
        );

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
