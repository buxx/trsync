pub mod error;
pub mod activity;
pub mod client;
pub mod config;
pub mod content;
pub mod instance;
pub mod job;
pub mod local;
pub mod remote;
pub mod security;
pub mod sync;
pub mod types;
pub mod user;
pub mod utils;

// This extension must match with Tracim content "filename"
pub const HTML_DOCUMENT_LOCAL_EXTENSION: &str = ".document.html";
