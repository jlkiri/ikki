use std::{collections::HashMap, path::PathBuf};

use bollard::{
    container::Config,
    models::{HostConfig, PortBinding},
};
use ikki_config::{Image, KeyValue, Service};

use crate::docker::DockerError;

#[derive(Debug)]
pub struct BuildOptions {
    pub path: Option<PathBuf>,
    pub pull: Option<String>,
    pub build_args: HashMap<String, String>,
    pub tag: String,
}

#[derive(Debug)]
pub struct RunOptions {
    pub container_name: String,
    pub image_name: String,
    pub env: Vec<String>,
    pub ports: Vec<String>,
}

pub fn build_options(image: &Image) -> Result<BuildOptions, DockerError> {
    let build_args = image
        .build_args
        .iter()
        .map(|kv| (kv.0.to_string(), kv.1.to_string()))
        .collect();

    if image.pull.is_none() && image.path.is_none() {
        return Err(DockerError::Settings(
            "missing either `path` or `pull` parameter".into(),
        ));
    }

    Ok(BuildOptions {
        build_args,
        pull: image.pull.clone(),
        path: image.path.clone(),
        tag: image.name.clone(),
    })
}

type ContainerPortConfig = String;

fn parse_port_binding(binding: String) -> (ContainerPortConfig, PortBinding) {
    let (host_port, container_address) = binding.split_once(':').unwrap_or(("", &binding));
    let (container_port, protocol) = container_address
        .split_once('/')
        .unwrap_or((container_address, "tcp"));

    let (host_ip, host_port) = if host_port.is_empty() {
        (None, None)
    } else {
        (Some("127.0.0.1".to_string()), Some(host_port.to_string()))
    };

    (
        format!("{}/{}", container_port, protocol),
        PortBinding { host_ip, host_port },
    )
}

fn create_ports_config(ports: Vec<String>) -> HashMap<String, Option<Vec<PortBinding>>> {
    let mut port_bindings = HashMap::new();
    for binding in ports {
        let (container, host) = parse_port_binding(binding);
        port_bindings.insert(container, Some(vec![host]));
    }
    port_bindings
}

fn create_env_config(env: Vec<KeyValue>) -> Vec<String> {
    env.into_iter()
        .map(|KeyValue(k, v)| format!("{}={}", k, v))
        .collect()
}

pub fn create_run_options(
    (container_name, image_name, service): (String, String, Service),
) -> RunOptions {
    RunOptions {
        container_name: container_name,
        env: create_env_config(service.env),
        ports: service.ports.unwrap_or_default(),
        image_name: image_name,
    }
}

pub fn create_container_config(
    container_name: &str,
    image_name: &str,
    service: Service,
) -> Config<String> {
    let mut config = Config::default();

    let options = create_run_options((container_name.to_string(), image_name.to_string(), service));

    config.image = Some(options.image_name);
    config.host_config = Some(HostConfig {
        port_bindings: Some(create_ports_config(options.ports)),
        ..Default::default()
    });
    config.env = Some(options.env);

    config
}
