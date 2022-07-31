use crate::{console, docker_config::*};
use bollard::{
    container::CreateContainerOptions,
    image::{BuildImageOptions, CreateImageOptions},
    Docker,
};
use futures::StreamExt;
use futures_util::TryStreamExt;
use ikki_config::*;
use indicatif::{MultiProgress, ProgressBar};
use std::{
    collections::{HashMap, HashSet},
    io::{self, Write},
};
use tar::Builder;
use thiserror::Error;
use tokio::task;
use tracing::debug;

static STATUS_DOWNLOADING: &str = "Downloading";

#[derive(Error, Debug)]
pub enum DockerError {
    #[error("Invalid settings: {0}")]
    Settings(String),
    #[error("Failed to archive a directory")]
    Archive(String),
    #[error("Docker daemon error: {0}")]
    DockerDaemonError(#[from] bollard::errors::Error),
}

pub async fn build_image(
    docker: Docker,
    image: Image,
    mp: MultiProgress,
) -> Result<(), DockerError> {
    debug!("building {}...", image.name);

    let pb = mp.add(console::default_build_progress_bar());
    pb.set_message(image.name.clone());

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

    let mut build_stream = docker.build_image(build_options, None, Some(tar.into()));

    let mut progresses = HashMap::new();

    let mut dl_pb: Option<ProgressBar> = None;

    while let Some(info) = build_stream.next().await {
        let info = info?;

        if let Some(status) = info.status {
            if status == STATUS_DOWNLOADING {
                if dl_pb.is_none() {
                    dl_pb = Some(mp.add(console::default_pull_progress_bar()));
                }

                let detail = info
                    .progress_detail
                    .and_then(|det| match (det.total, det.current) {
                        (Some(total), Some(current)) => Some((total, current)),
                        _ => None,
                    });

                let id = info.id.unwrap_or_default();

                dl_pb
                    .as_ref()
                    .unwrap()
                    .set_message(format!("Pulling missing layers..."));

                if let Some((total, current)) = detail {
                    let e = progresses.entry(id).or_insert((total, current));
                    *e = (e.0, current);

                    let all_total: i64 = progresses.values().map(|(t, _)| t).sum();
                    let all_current: i64 = progresses.values().map(|(_, c)| c).sum();
                    dl_pb.as_ref().unwrap().set_length(all_total as u64);
                    dl_pb.as_ref().unwrap().set_position(all_current as u64);
                }
            }
        }

        pb.tick();
    }

    if let Some(dl_pb) = dl_pb {
        dl_pb.finish_and_clear();
    }
    pb.finish_and_clear();

    Ok(())
}

pub async fn pull_image(
    docker: Docker,
    image: Image,
    mp: MultiProgress,
) -> Result<(), DockerError> {
    let name = image.pull.unwrap_or("<unknown>".into());
    debug!("pulling {}...", name);

    let image_list = docker.list_images::<String>(None).await?;
    if image_list
        .iter()
        .any(|img| img.repo_tags.iter().any(|tag| tag.contains(&name)))
    {
        debug!("image `{}` already exists, skipping", name);
        println!("Image `{}` already exists and/or is up-to-date", name);
        return Ok(());
    }

    let pb = mp.add(console::default_pull_progress_bar());

    let mut pull_stream = docker.create_image(
        Some(CreateImageOptions {
            from_image: name.clone(),
            ..Default::default()
        }),
        None,
        None,
    );

    let mut progresses = HashMap::new();

    while let Some(info) = pull_stream.next().await {
        let info = info?;

        if let Some(status) = info.status {
            if status == STATUS_DOWNLOADING {
                let detail = info
                    .progress_detail
                    .and_then(|det| match (det.total, det.current) {
                        (Some(total), Some(current)) => Some((total, current)),
                        _ => None,
                    });

                let id = info.id.unwrap_or_default();

                pb.set_message(format!("Pulling {}", name));

                if let Some((total, current)) = detail {
                    let e = progresses.entry(id).or_insert((total, current));
                    *e = (e.0, current);

                    let all_total: i64 = progresses.values().map(|(t, _)| t).sum();
                    let all_current: i64 = progresses.values().map(|(_, c)| c).sum();
                    pb.set_length(all_total as u64);
                    pb.set_position(all_current as u64);
                }
            }
        }
    }

    pb.finish_and_clear();

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

    let config = create_container_config(&container_name, &image_name, service);

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
