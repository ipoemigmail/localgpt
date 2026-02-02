pub mod ask;
pub mod chat;
pub mod config;
pub mod daemon;
pub mod memory;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "localgpt")]
#[command(author, version, about = "A lightweight, local-only AI assistant")]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Path to config file
    #[arg(short, long, global = true, env = "LOCALGPT_CONFIG")]
    pub config: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start an interactive chat session
    Chat(chat::ChatArgs),

    /// Ask a single question
    Ask(ask::AskArgs),

    /// Manage the daemon
    Daemon(daemon::DaemonArgs),

    /// Memory operations
    Memory(memory::MemoryArgs),

    /// Configuration management
    Config(config::ConfigArgs),
}
