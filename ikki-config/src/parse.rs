use crate::{deps::parse_deps, parse_image_config, Image, ImageConfig};
use kdl::{KdlDocument, KdlError};
use toposort::Toposort;

use thiserror::Error;

pub type BuildOrder = Vec<Vec<String>>;

#[derive(Error, Debug)]
pub enum IkkiConfigError {
    #[error("Invalid Ikki configuration: {0}")]
    InvalidConfiguration(String),
    #[error("Configuration deserialization failed: {0}")]
    Knuffel(#[from] knuffel::Error),
}

#[derive(Debug)]
pub struct IkkiConfig {
    image_config: ImageConfig,
    build_order: Vec<Vec<String>>,
}

impl IkkiConfig {
    pub fn images(&self) -> &Vec<Image> {
        &self.image_config.images.images
    }

    pub fn find_image(&self, name: &str) -> Option<&Image> {
        self.image_config
            .images
            .images
            .iter()
            .find(|img| img.name == name)
    }

    pub fn build_order(&self) -> BuildOrder {
        self.build_order.clone()
    }
}

pub fn parse(filename: &str, input: &str) -> Result<IkkiConfig, IkkiConfigError> {
    let doc: KdlDocument = input
        .parse()
        .map_err(|e: KdlError| IkkiConfigError::InvalidConfiguration(e.to_string()))?;

    let images = doc
        .get("images")
        .ok_or(IkkiConfigError::InvalidConfiguration(
            "missing `images` configuration".to_string(),
        ))?;

    let dependencies = doc.get("dependencies");
    let image_config = parse_image_config(filename, &images.to_string())?;
    let build_order = dependencies
        .map(|deps| {
            let dag = parse_deps(deps);
            dag.toposort().unwrap_or_default()
        })
        .unwrap_or_else(|| {
            let image_names = image_config.image_names();
            vec![image_names]
        });

    Ok(IkkiConfig {
        image_config,
        build_order,
    })
}
