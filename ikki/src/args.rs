use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

const DEFAULT_CONFIG_FILE: &str = "ikki.kdl";

/// Unison orchestrates Docker image builds and container launches
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
pub struct Unison {
    /// Unison subcommand
    #[clap(subcommand)]
    pub command: Command,
    /// Path to Unison configuration file
    #[clap(long, short, value_parser, default_value = DEFAULT_CONFIG_FILE)]
    pub file: PathBuf,
}

/// Unison subcommand
#[derive(Debug, Subcommand)]
pub enum Command {
    Build(BuildCmdArgs),
    /// Build (or pull) all images and start the services
    Up(UpOptions),
    Explain,
}

#[derive(Args, Debug)]
pub struct UpOptions {
    #[clap(long)]
    /// Watch for FS changes and Docker events to trigger necessary rebuilds and restarts
    watch: bool,
}

#[derive(Debug, Args)]
pub struct BuildCmdArgs {
    #[clap(value_parser)]
    name: Option<String>,
}
