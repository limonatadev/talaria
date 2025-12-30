//! Core Hermes API client and typed models derived from the OpenAPI spec.
//! This crate is consumed by both the CLI and TUI frontends.

pub mod camera;
pub mod client;
pub mod config;
pub mod error;
pub mod images;
pub mod models;
pub mod supabase;

pub use crate::client::HermesClient;
pub use crate::config::Config;
pub use crate::error::{Error, Result};
pub use crate::models::*;
