use anyhow::Result;
use clap::{Args, Subcommand};

use localgpt::config::Config;

#[derive(Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommands,
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Show current configuration
    Show {
        /// Output format: toml (default) or json
        #[arg(short, long, default_value = "toml")]
        format: String,
    },

    /// Get a configuration value
    Get {
        /// Config key (e.g., agent.default_model)
        key: String,
    },

    /// Set a configuration value
    Set {
        /// Config key (e.g., agent.default_model)
        key: String,

        /// Value to set
        value: String,
    },

    /// Show config file path
    Path,

    /// Initialize default config file
    Init {
        /// Overwrite existing config
        #[arg(short, long)]
        force: bool,
    },
}

pub async fn run(args: ConfigArgs) -> Result<()> {
    match args.command {
        ConfigCommands::Show { format } => show_config(&format),
        ConfigCommands::Get { key } => get_config(&key),
        ConfigCommands::Set { key, value } => set_config(&key, &value),
        ConfigCommands::Path => show_path(),
        ConfigCommands::Init { force } => init_config(force),
    }
}

fn show_config(format: &str) -> Result<()> {
    let config = Config::load()?;

    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&config)?;
            println!("{}", json);
        }
        _ => {
            let toml = toml::to_string_pretty(&config)?;
            println!("{}", toml);
        }
    }

    Ok(())
}

fn get_config(key: &str) -> Result<()> {
    let config = Config::load()?;
    let value = config.get_value(key)?;
    println!("{}", value);
    Ok(())
}

fn set_config(key: &str, value: &str) -> Result<()> {
    let mut config = Config::load()?;
    config.set_value(key, value)?;
    config.save()?;
    println!("Set {} = {}", key, value);
    Ok(())
}

fn show_path() -> Result<()> {
    let path = Config::config_path()?;
    println!("{}", path.display());
    Ok(())
}

fn init_config(force: bool) -> Result<()> {
    let path = Config::config_path()?;

    if path.exists() && !force {
        anyhow::bail!(
            "Config file already exists at {}. Use --force to overwrite.",
            path.display()
        );
    }

    // Create parent directories
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let default_config = Config::default();
    default_config.save()?;

    println!("Created config file at {}", path.display());
    Ok(())
}
