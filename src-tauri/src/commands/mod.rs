//! Tauri command modules — split out of `lib.rs` by domain.
//!
//! Each submodule holds a coherent group of `#[tauri::command]` handlers; the
//! crate root composes them all into a single `tauri::generate_handler!`
//! list. The split is pure organization — every command's signature,
//! behavior, and registered name is unchanged from the pre-split lib.rs.

pub(crate) mod api_keys;
pub(crate) mod chat;
pub(crate) mod context;
pub(crate) mod documents;
pub(crate) mod employees;
pub(crate) mod enps;
pub(crate) mod import;
pub(crate) mod license;
pub(crate) mod performance;
pub(crate) mod system;
