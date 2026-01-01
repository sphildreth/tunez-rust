//! External plugin support for Tunez music player.
//!
//! This crate provides:
//! - A JSON-based protocol for communicating with external plugin processes
//! - An exec-based plugin host that spawns and manages plugin processes
//! - An adapter that implements the `Provider` trait for external plugins
//!
//! # Plugin Protocol
//!
//! Plugins communicate with Tunez via JSON messages over stdin/stdout:
//! - Tunez sends [`PluginRequest`] messages (one per line) to the plugin's stdin
//! - The plugin responds with [`PluginResponse`] messages (one per line) on stdout
//!
//! # Example Plugin (pseudocode)
//!
//! ```text
//! while (line = read_stdin()):
//!     request = json_parse(line)
//!     if request.method == "Initialize":
//!         response = {
//!             "id": request.id,
//!             "result": {
//!                 "status": "Initialized",
//!                 "id": "my-plugin",
//!                 "name": "My Plugin",
//!                 "version": "1.0.0",
//!                 "protocol_version": 1
//!             }
//!         }
//!     elif request.method == "SearchTracks":
//!         # ... handle search
//!     write_stdout(json_stringify(response) + "\n")
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use tunez_plugin::{ExecPluginProvider, PluginConfig};
//! use std::path::PathBuf;
//!
//! let config = PluginConfig {
//!     executable: PathBuf::from("/path/to/my-plugin"),
//!     args: vec![],
//!     working_dir: None,
//!     env: vec![],
//! };
//!
//! let provider = ExecPluginProvider::new(config)?;
//! // provider now implements tunez_core::provider::Provider
//! ```

mod adapter;
mod host;
pub mod protocol;

pub use adapter::ExecPluginProvider;
pub use host::{ExecPluginHost, PluginConfig, PluginHostError};
pub use protocol::{
    PluginError, PluginErrorKind, PluginInfo, PluginMethod, PluginRequest, PluginResponse,
    PluginResult, PROTOCOL_VERSION,
};
