pub mod archive;
pub mod engine;
pub mod exclusions;
pub mod i18n;
mod metadata;
pub mod model;
pub mod plain;
pub mod platform;
pub mod protocol;
pub mod remote;
pub mod resources;
pub mod security;
#[cfg(windows)]
pub mod service;
pub mod store;
#[cfg(windows)]
pub mod tray;
#[cfg(unix)]
pub mod unix_security;
#[cfg(unix)]
pub mod unix_transport;
#[cfg(windows)]
pub mod updater;
pub mod updates;
#[cfg(windows)]
pub mod vss;
#[cfg(unix)]
#[path = "vss_unavailable.rs"]
pub mod vss;
pub mod worker;
