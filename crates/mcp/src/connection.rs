//! `<app-data>/mcp-connection.json`: where the studio's MCP server listens and its token.
//!
//! Written by the studio (mode 0600 on Unix) while its server runs, read by `labely mcp`
//! to bridge, and removed on shutdown.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::McpError;

/// Default studio MCP port.
pub const DEFAULT_PORT: u16 = 7878;

/// Contents of the connection file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionInfo {
    /// e.g. `http://127.0.0.1:7878/mcp`
    pub url: String,
    /// Bearer token (without `Bearer `).
    pub token: String,
    /// Process id of the studio.
    pub pid: u32,
}

/// Per-user data directory: `$LABELY_DATA_DIR`, else the platform default.
pub fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("LABELY_DATA_DIR") {
        return PathBuf::from(d);
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Labely")
    } else if cfg!(windows) {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or(home)
            .join("Labely")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"))
            .join("labely")
    }
}

/// Default connection file path.
pub fn default_path() -> PathBuf {
    data_dir().join("mcp-connection.json")
}

/// 32 random bytes as hex.
pub fn new_token() -> Result<String, McpError> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).map_err(|e| McpError::Connection(format!("no randomness: {e}")))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// Write `info` to `path`, readable by the owner only.
pub fn write(path: &Path, info: &ConnectionInfo) -> Result<(), McpError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec_pretty(info).map_err(|e| McpError::Connection(e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        use std::io::Write;
        let mut f = opts.open(&tmp)?;
        f.write_all(&json)?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Read the connection file; `Ok(None)` when it does not exist.
pub fn read(path: &Path) -> Result<Option<ConnectionInfo>, McpError> {
    match std::fs::read(path) {
        Ok(b) => serde_json::from_slice(&b)
            .map(Some)
            .map_err(|e| McpError::Connection(format!("{}: {e}", path.display()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Remove the file if it still belongs to this process.
pub fn remove_own(path: &Path) {
    if let Ok(Some(info)) = read(path)
        && info.pid == std::process::id()
    {
        std::fs::remove_file(path).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_mode() {
        let dir = tempfile_dir();
        let path = dir.join("c.json");
        let info = ConnectionInfo {
            url: "http://127.0.0.1:1/mcp".into(),
            token: new_token().unwrap(),
            pid: std::process::id(),
        };
        assert_eq!(info.token.len(), 64);
        write(&path, &info).unwrap();
        assert_eq!(read(&path).unwrap(), Some(info));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        remove_own(&path);
        assert_eq!(read(&path).unwrap(), None);
        std::fs::remove_dir_all(dir).ok();
    }

    fn tempfile_dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("labely-conn-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}
