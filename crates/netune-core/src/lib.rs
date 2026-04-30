//! netune-core: data models, error types, and configuration.

pub mod config;
pub mod error;
pub mod models;
pub mod traits;

pub use error::{NetuneError, Result};
