//! DeepSeek Agent - A minimal CLI agent using DeepSeek API with shell execution capability.
//!
//! This library provides the core functionality for the DeepSeek agent, including:
//! - API client with retry logic
//! - History management with token estimation
//! - Shell command execution
//! - Streaming response processing
//! - Session management and restart

// Déclaration des modules
pub mod agent;
pub mod api;
pub mod api_client;
pub mod config;
pub mod history;
pub mod interrupt;
pub mod session;
pub mod shell;
pub mod streaming;
pub mod token_management;
pub mod ui;

// Ré-export des types principaux pour une utilisation simplifiée
pub use agent::Agent;
pub use api::*;
pub use api_client::ApiClient;
pub use config::Config;
pub use history::HistoryManager;
pub use session::{check_and_restart_if_needed, RestartSessionError};
pub use shell::ShellExecutor;
pub use token_management::*;
pub use ui::{colors_enabled, init_colors, MessageFormatter};