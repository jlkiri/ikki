use crate::{
    args::*,
};

use clap::Parser;
use docker::DockerError;
use miette::{self, Diagnostic};
use std::path::Path;

use thiserror::Error;
use tokio::{fs};
use tracing::{debug, error};
use tracing_subscriber::EnvFilter;
use unison_config::*;

mod args;
mod builder;
mod cmd;
mod docker;
mod docker_config;
mod explain;
mod listeners;
mod supervisor;

type Result<T> = miette::Result<T>;

#[derive(Debug, Error, Diagnostic)]
pub enum UnisonError {
    #[error("Image does not exist: {0}")]
    NoSuchImage(String),
    #[error("FS change watcher failed")]
    FileWatcher,
    #[error("No Unison configuration file found at: {0}")]
    NoConfig(String),
    #[error("Unison configuration error")]
    Config(#[from] unison_config::UnisonConfigError),
    #[error("Docker build failed")]
    Build(#[from] DockerError),
    #[error("Unexpected error: {0}")]
    Other(String),
}

fn setup() {
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .expect("failed to create EnvFilter");

    tracing_subscriber::fmt()
        .with_env_filter(filter_layer)
        .init();
}

async fn read_config<P>(file: P) -> std::result::Result<UnisonConfig, UnisonError>
where
    P: AsRef<Path>,
{
    let path = file.as_ref().to_string_lossy();
    let input = fs::read_to_string(&file)
        .await
        .or(Err(UnisonError::NoConfig(path.to_string())))?;
    let config: UnisonConfig = unison_config::parse(&path, &input)?;
    Ok(config)
}

#[tokio::main]
async fn main() -> Result<()> {
    setup();

    debug!("initialized tracing_subscriber");

    let args = Unison::parse();
    let config = read_config(args.file.clone()).await?;

    debug!("loaded configuration from {}", args.file.display());

    match args.command {
        Command::Up(_opts) => cmd::up(config).await?,
        Command::Explain => cmd::explain(config).await?,
        _ => unimplemented!(),
    }

    Ok(())
}
