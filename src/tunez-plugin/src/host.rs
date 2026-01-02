//! Exec-based plugin host that communicates with external processes via JSON over stdio.

use crate::protocol::{
    PluginInfo, PluginMethod, PluginRequest, PluginResponse, PluginResult, PROTOCOL_VERSION,
};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use thiserror::Error;

/// Errors from plugin host operations.
#[derive(Debug, Error)]
pub enum PluginHostError {
    #[error("failed to spawn plugin process: {0}")]
    SpawnFailed(std::io::Error),
    #[error("plugin process has no stdin")]
    NoStdin,
    #[error("plugin process has no stdout")]
    NoStdout,
    #[error("failed to write to plugin: {0}")]
    WriteError(std::io::Error),
    #[error("failed to read from plugin: {0}")]
    ReadError(std::io::Error),
    #[error("failed to parse plugin response: {0}")]
    ParseError(serde_json::Error),
    #[error("plugin returned error: {0}")]
    PluginError(String),
    #[error("protocol version mismatch: expected {expected}, got {actual}")]
    ProtocolMismatch { expected: u32, actual: u32 },
    #[error("unexpected response type for method")]
    UnexpectedResponse,
    #[error("request/response ID mismatch: sent {sent}, received {received}")]
    IdMismatch { sent: u64, received: u64 },
    #[error("plugin process terminated unexpectedly")]
    ProcessTerminated,
}

/// Configuration for an external plugin.
#[derive(Debug, Clone)]
pub struct PluginConfig {
    /// Path to the plugin executable.
    pub executable: PathBuf,
    /// Arguments to pass to the plugin.
    pub args: Vec<String>,
    /// Working directory for the plugin process.
    pub working_dir: Option<PathBuf>,
    /// Environment variables to set for the plugin.
    pub env: Vec<(String, String)>,
}

/// Host for an external plugin process.
pub struct ExecPluginHost {
    config: PluginConfig,
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<ChildStdin>>,
    stdout: Mutex<Option<BufReader<ChildStdout>>>,
    request_id: AtomicU64,
    info: Mutex<Option<PluginInfo>>,
}

impl ExecPluginHost {
    /// Create a new plugin host with the given configuration.
    pub fn new(config: PluginConfig) -> Self {
        Self {
            config,
            child: Mutex::new(None),
            stdin: Mutex::new(None),
            stdout: Mutex::new(None),
            request_id: AtomicU64::new(1),
            info: Mutex::new(None),
        }
    }

    /// Start the plugin process and initialize it.
    pub fn start(&self) -> Result<PluginInfo, PluginHostError> {
        let mut cmd = Command::new(&self.config.executable);
        cmd.args(&self.config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        if let Some(ref dir) = self.config.working_dir {
            cmd.current_dir(dir);
        }

        for (key, value) in &self.config.env {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn().map_err(PluginHostError::SpawnFailed)?;

        let stdin = child.stdin.take().ok_or(PluginHostError::NoStdin)?;
        let stdout = child.stdout.take().ok_or(PluginHostError::NoStdout)?;

        *self.child.lock().unwrap() = Some(child);
        *self.stdin.lock().unwrap() = Some(stdin);
        *self.stdout.lock().unwrap() = Some(BufReader::new(stdout));

        // Initialize the plugin
        let info = self.initialize()?;
        *self.info.lock().unwrap() = Some(info.clone());

        Ok(info)
    }

    /// Stop the plugin process gracefully.
    pub fn stop(&self) -> Result<(), PluginHostError> {
        // Try to send shutdown request
        if self.stdin.lock().unwrap().is_some() {
            let _ = self.send_request(PluginMethod::Shutdown);
        }

        // Terminate the process if still running
        if let Some(mut child) = self.child.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        *self.stdin.lock().unwrap() = None;
        *self.stdout.lock().unwrap() = None;
        *self.info.lock().unwrap() = None;

        Ok(())
    }

    /// Check if the plugin process is running.
    pub fn is_running(&self) -> bool {
        self.child
            .lock()
            .unwrap()
            .as_mut()
            .map(|c| c.try_wait().ok().flatten().is_none())
            .unwrap_or(false)
    }

    /// Get the plugin info (available after start).
    pub fn info(&self) -> Option<PluginInfo> {
        self.info.lock().unwrap().clone()
    }

    /// Send a request to the plugin and receive a response.
    pub fn send_request(&self, method: PluginMethod) -> Result<PluginResult, PluginHostError> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = PluginRequest { id, method };

        // Serialize and write request
        let json = serde_json::to_string(&request).map_err(PluginHostError::ParseError)?;

        {
            let mut stdin_guard = self.stdin.lock().unwrap();
            let stdin = stdin_guard
                .as_mut()
                .ok_or(PluginHostError::ProcessTerminated)?;
            writeln!(stdin, "{}", json).map_err(PluginHostError::WriteError)?;
            stdin.flush().map_err(PluginHostError::WriteError)?;
        }

        // Read response
        let response_line = {
            let mut stdout_guard = self.stdout.lock().unwrap();
            let stdout = stdout_guard
                .as_mut()
                .ok_or(PluginHostError::ProcessTerminated)?;
            let mut line = String::new();
            stdout
                .read_line(&mut line)
                .map_err(PluginHostError::ReadError)?;
            if line.is_empty() {
                return Err(PluginHostError::ProcessTerminated);
            }
            line
        };

        let response: PluginResponse =
            serde_json::from_str(&response_line).map_err(PluginHostError::ParseError)?;

        if response.id != id {
            return Err(PluginHostError::IdMismatch {
                sent: id,
                received: response.id,
            });
        }

        // Check for error results
        if let PluginResult::Error(err) = &response.result {
            return Err(PluginHostError::PluginError(err.message.clone()));
        }

        Ok(response.result)
    }

    fn initialize(&self) -> Result<PluginInfo, PluginHostError> {
        let result = self.send_request(PluginMethod::Initialize)?;
        match result {
            PluginResult::Initialized(info) => {
                if info.protocol_version != PROTOCOL_VERSION {
                    return Err(PluginHostError::ProtocolMismatch {
                        expected: PROTOCOL_VERSION,
                        actual: info.protocol_version,
                    });
                }
                tracing::info!(
                    plugin_id = %info.id,
                    plugin_name = %info.name,
                    plugin_version = %info.version,
                    "Plugin initialized"
                );
                Ok(info)
            }
            _ => Err(PluginHostError::UnexpectedResponse),
        }
    }
}

impl Drop for ExecPluginHost {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[cfg(unix)]
    fn create_test_plugin_script() -> tempfile::TempPath {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"#!/bin/bash
while IFS= read -r line; do
    id=$(echo "$line" | grep -o '"id":[0-9]*' | cut -d: -f2)
    echo '{{"id":'$id',"result":{{"status":"Initialized","id":"test","name":"Test","version":"1.0.0","protocol_version":1}}}}'
done
"#
        )
        .unwrap();
        file.flush().unwrap();

        // Make executable
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(file.path(), std::fs::Permissions::from_mode(0o755)).unwrap();

        file.into_temp_path()
    }

    #[test]
    #[cfg(unix)]
    fn plugin_config_creates_correctly() {
        let config = PluginConfig {
            executable: PathBuf::from("/usr/bin/test-plugin"),
            args: vec!["--config".to_string(), "test.toml".to_string()],
            working_dir: None,
            env: vec![("PLUGIN_DEBUG".to_string(), "1".to_string())],
        };
        assert_eq!(config.args.len(), 2);
        assert_eq!(config.env.len(), 1);
    }

    #[test]
    #[cfg(unix)]
    fn plugin_handshake_works() {
        let script = create_test_plugin_script();
        let config = PluginConfig {
            executable: script.to_path_buf(),
            args: vec![],
            working_dir: None,
            env: vec![],
        };
        
        let host = ExecPluginHost::new(config);
        let info = host.start().expect("failed to start plugin");
        
        assert_eq!(info.id, "test");
        assert_eq!(info.version, "1.0.0");
        
        host.stop().expect("failed to stop");
    }
}
