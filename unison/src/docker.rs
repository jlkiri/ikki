use crate::docker_config::*;
use bollard::{
    container::CreateContainerOptions,
    image::{BuildImageOptions, CreateImageOptions},
    Docker,
};
use futures_util::TryStreamExt;
use std::io::{self, Write};
use tar::Builder;
use thiserror::Error;
use tokio::task;
use tracing::debug;
use unison_config::*;

fn clear_line() {
    use crossterm::{cursor::MoveToColumn, execute, terminal::Clear, terminal::ClearType};
    execute!(io::stdout(), MoveToColumn(0)).expect("Failed to clear line");
    execute!(io::stdout(), Clear(ClearType::CurrentLine)).expect("Failed to clear line");
}

#[derive(Error, Debug)]
pub enum DockerError {
    #[error("Invalid settings: {0}")]
    Settings(String),
    #[error("Failed to archive a directory")]
    Archive(String),
    #[error("Docker daemon error: {0}")]
    DockerDaemonError(#[from] bollard::errors::Error),
}

pub async fn build_image(docker: Docker, image: Image) -> Result<(), DockerError> {
    debug!("building {}...", image.name);

    println!("Building image `{}`...", image.name);

    let build_opts = build_options(&image)?;

    let build_path = build_opts.path.ok_or(DockerError::Settings(format!(
        "missing image build path for image `{}`",
        image.name
    )))?;

    let archive_task = task::spawn_blocking(|| {
        let mut buf = vec![];
        let mut tar = Builder::new(&mut buf);
        tar.append_dir_all("", build_path)?;
        tar.into_inner().cloned()
    });

    let tar = archive_task
        .await
        .map(|res| res.map_err(|e| DockerError::Archive(e.to_string())))
        .map_err(|e| DockerError::Archive(e.to_string()))??;

    let build_options = BuildImageOptions {
        dockerfile: "Dockerfile".to_string(),
        t: image.name.clone(),
        buildargs: build_opts.build_args,
        rm: true,
        ..Default::default()
    };

    docker
        .build_image(build_options, None, Some(tar.into()))
        .try_collect::<Vec<_>>()
        .await?;

    println!("Finished building `{}`", image.name);

    Ok(())
}

pub async fn pull_image(docker: Docker, image: Image) -> Result<(), DockerError> {
    let name = image.pull.unwrap_or("<unknown>".into());
    debug!("pulling {}...", name);

    println!(
        "Checking if image `{}` needs to be built or pulled from registry...",
        name
    );

    let image_list = docker.list_images::<String>(None).await?;
    if image_list
        .iter()
        .any(|img| img.repo_tags.iter().any(|tag| tag.contains(&name)))
    {
        debug!("image `{}` already exists, skipping", name);
        println!("Image `{}` already exists and/or is up-to-date", name);
        return Ok(());
    }

    println!(
        "Image `{}` not found locally. Pulling from registry...",
        name
    );

    docker
        .create_image(
            Some(CreateImageOptions {
                from_image: name.clone(),
                ..Default::default()
            }),
            None,
            None,
        )
        .try_collect::<Vec<_>>()
        .await?;

    println!("Finished pulling `{}` from registry", name);

    Ok(())
}

pub async fn run(
    docker: Docker,
    container_name: String,
    image_name: String,
    service: Service,
) -> Result<String, DockerError> {
    debug!("starting {}...", container_name);
    println!("Creating container `{}`...", container_name);

    let options = CreateContainerOptions {
        name: container_name.to_string(),
    };

    let config = create_container_config(&image_name, service);

    let id = docker.create_container(Some(options), config).await?.id;
    println!("Created container `{}`", container_name);

    println!("Starting container `{}`...", container_name);
    docker.start_container::<String>(&id, None).await?;
    println!("Started container `{}` ({})", container_name, id);

    debug!("started container {} ({})", container_name, id);

    Ok(id)
}

pub async fn remove_container(docker: Docker, id: &str) -> Result<(), DockerError> {
    docker.stop_container(id, None).await?;
    docker.remove_container(id, None).await?;

    Ok(())
}
