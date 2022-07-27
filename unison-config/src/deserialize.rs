#![allow(dead_code)]

use std::path::PathBuf;

#[derive(Debug, Clone, knuffel::Decode)]
pub struct KeyValue(
    #[knuffel(argument)] pub String,
    #[knuffel(argument)] pub String,
);

#[derive(Debug, Clone, knuffel::Decode)]
pub struct Secret {
    #[knuffel(property)]
    pub id: String,
    #[knuffel(property)]
    pub src: PathBuf,
}

#[derive(Debug, Clone, knuffel::Decode)]
pub struct Mount {
    #[knuffel(property(name = "type"))]
    pub mount_type: String,
    #[knuffel(property)]
    pub src: PathBuf,
    #[knuffel(property)]
    pub dest: PathBuf,
}

#[derive(Debug, Clone, knuffel::Decode)]
pub struct Service {
    #[knuffel(child, unwrap(arguments))]
    pub ports: Option<Vec<String>>,
    #[knuffel(children(name = "env"))]
    pub env: Vec<KeyValue>,
    #[knuffel(child, unwrap(argument))]
    pub user: Option<String>,
    #[knuffel(children(name = "mount"))]
    pub mounts: Vec<Mount>,
    #[knuffel(child, unwrap(arguments))]
    pub networks: Option<Vec<String>>,
}

#[derive(Debug, knuffel::Decode)]
pub struct BuildArg {
    #[knuffel(arguments)]
    pub values: Vec<String>,
}

#[derive(Debug, Clone, knuffel::Decode)]
pub struct Image {
    #[knuffel(property)]
    pub path: Option<PathBuf>,
    #[knuffel(property)]
    pub file: Option<PathBuf>,
    #[knuffel(property)]
    pub output: Option<PathBuf>,
    #[knuffel(property)]
    pub pull: Option<String>,
    #[knuffel(children(name = "build-arg"))]
    pub build_args: Vec<KeyValue>,
    #[knuffel(child)]
    pub service: Option<Service>,
    #[knuffel(child)]
    pub secret: Option<Secret>,
    #[knuffel(argument)]
    pub name: String,
}

#[derive(Debug, knuffel::Decode)]
pub struct Images {
    #[knuffel(children(name = "image"))]
    pub images: Vec<Image>,
}

#[derive(Debug, knuffel::Decode)]
pub struct ImageConfig {
    #[knuffel(child)]
    pub images: Images,
}

impl ImageConfig {
    pub fn image_names(&self) -> Vec<String> {
        self.images
            .images
            .iter()
            .map(|img| img.name.clone())
            .collect()
    }
}

pub fn parse_image_config(filename: &str, input: &str) -> Result<ImageConfig, knuffel::Error> {
    knuffel::parse::<ImageConfig>(filename, input)
}
