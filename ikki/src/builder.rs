use std::ops::Mul;

use bollard::Docker;
use futures::prelude::*;
use futures::stream::FuturesUnordered;
use ikki_config::{BuildOrder, IkkiConfig, Image};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::sync::oneshot::Sender;
use tokio::sync::{mpsc, oneshot};
use tokio::task;
use tracing::debug;

use crate::docker::DockerError;
use crate::{docker, IkkiError};

type ImageName = String;

#[derive(Debug)]
pub enum BuildResult {
    Success,
    Error(IkkiError),
}

#[derive(Debug)]
pub enum RunResult {
    Success(ContainerIds),
    Error(IkkiError),
}

#[derive(Debug)]
pub enum StopResult {
    Success,
    Error(IkkiError),
}

type BuildResultSender = oneshot::Sender<BuildResult>;
type RunResultSender = oneshot::Sender<RunResult>;
type StopResultSender = oneshot::Sender<StopResult>;

type ContainerIds = Vec<String>;

#[derive(Debug)]
pub enum Command {
    Build((ImageName, BuildResultSender)),
    Run((ImageName, RunResultSender)),
    BuildAll(BuildResultSender),
    RunAll(RunResultSender),
    StopAll((ContainerIds, StopResultSender)),
}

pub type CommandReceiver = mpsc::Receiver<Command>;
pub type CommandSender = mpsc::Sender<Command>;

struct Builder {
    receiver: CommandReceiver,
    client: Docker,
    config: IkkiConfig,
}

async fn create_docker_job(
    docker: Docker,
    image: Image,
    mp: MultiProgress,
) -> Result<(), DockerError> {
    if let Some(_pull) = &image.pull {
        docker::pull_image(docker, image, mp).await?;
    } else if let Some(_path) = &image.path {
        docker::build_image(docker, image, mp).await?;
    }
    Ok(())
}

impl Builder {
    fn new(receiver: CommandReceiver, client: Docker, config: IkkiConfig) -> Self {
        Self {
            receiver,
            client,
            config,
        }
    }

    fn report_build_result(&self, sender: Sender<BuildResult>, result: Result<(), IkkiError>) {
        match result {
            Ok(()) => {
                let _ = sender.send(BuildResult::Success);
            }
            Err(e) => {
                let _ = sender.send(BuildResult::Error(e));
            }
        }
    }

    fn report_run_result(
        &self,
        sender: Sender<RunResult>,
        result: Result<ContainerIds, IkkiError>,
    ) {
        match result {
            Ok(ids) => {
                let _ = sender.send(RunResult::Success(ids));
            }
            Err(e) => {
                let _ = sender.send(RunResult::Error(e));
            }
        }
    }

    fn report_stop_result(&self, sender: Sender<StopResult>, result: Result<(), IkkiError>) {
        match result {
            Ok(()) => {
                let _ = sender.send(StopResult::Success);
            }
            Err(e) => {
                let _ = sender.send(StopResult::Error(e));
            }
        }
    }

    async fn handle_command(&self, cmd: Command) {
        match cmd {
            Command::BuildAll(sender) => {
                let result = self.full_build().await;
                self.report_build_result(sender, result)
            }
            Command::RunAll(sender) => {
                let result = self.full_run().await;
                self.report_run_result(sender, result)
            }
            Command::Build((image_name, sender)) => {
                let result = self.build_dependers(&image_name).await;
                self.report_build_result(sender, result)
            }
            Command::Run((image_name, sender)) => {
                let result = self.run_dependers(&image_name).await;
                self.report_run_result(sender, result)
            }
            Command::StopAll((ids, sender)) => {
                let result = self.stop_all(ids).await;
                self.report_stop_result(sender, result)
            }
        }
    }

    async fn ordered_build(&self, order: BuildOrder) -> Result<(), IkkiError> {
        debug!("executing build jobs in configured order");
        let mp = MultiProgress::new();

        for chunk in order {
            // Concurrently run builds/pulls in a single chunk because they do not depend on each other.
            let queue = FuturesUnordered::new();

            for image_name in chunk {
                let image = self
                    .config
                    .find_image(&image_name)
                    .cloned()
                    .ok_or(IkkiError::NoSuchImage(image_name))?;
                let job = create_docker_job(self.client.clone(), image, mp.clone());
                queue.push(job);
            }

            queue
                .collect::<Vec<_>>()
                .await
                .into_iter()
                .collect::<Result<Vec<()>, DockerError>>()?;
        }

        mp.clear().expect("failed to clear multiple progress bars");

        debug!("all build jobs finished successfully");
        Ok(())
    }

    async fn ordered_run(&self, order: BuildOrder) -> Result<ContainerIds, IkkiError> {
        debug!("executing run jobs in configured order");
        let mut container_ids = vec![];

        for chunk in order {
            // Concurrently run builds/pulls in a single chunk because they do not depend on each other.
            let queue = FuturesUnordered::new();

            for image_name in chunk {
                let image = self
                    .config
                    .find_image(&image_name)
                    .cloned()
                    .ok_or(IkkiError::NoSuchImage(image_name))?;
                if let Some(service) = image.service {
                    let image_name = if let Some(name) = image.pull {
                        name
                    } else {
                        image.name.clone()
                    };
                    let container_name = image.name;
                    let job = docker::run(self.client.clone(), container_name, image_name, service);
                    queue.push(job);
                }
            }

            let ids = queue
                .collect::<Vec<_>>()
                .await
                .into_iter()
                .collect::<Result<Vec<String>, DockerError>>()?;

            container_ids.extend_from_slice(&ids);
        }
        debug!("all run jobs finished successfully");
        Ok(container_ids)
    }

    fn dependent_images_of(&self, name: &str) -> Vec<Vec<String>> {
        let mut dependent_images: Vec<Vec<String>> = self
            .config
            .build_order()
            .into_iter()
            .skip_while(|chunk| !chunk.contains(&name.to_string()))
            .collect();

        // We want to avoid building anything in the same chunk as the target image
        // but still need to build it
        let chunk = &mut dependent_images[0];
        *chunk = vec![name.to_string()];

        dependent_images
    }

    async fn build_dependers(&self, name: &str) -> Result<(), IkkiError> {
        let dependers = self.dependent_images_of(name);
        self.ordered_build(dependers).await
    }

    async fn full_build(&self) -> Result<(), IkkiError> {
        self.ordered_build(self.config.build_order()).await
    }

    async fn run_dependers(&self, name: &str) -> Result<ContainerIds, IkkiError> {
        let dependers = self.dependent_images_of(name);
        self.ordered_run(dependers).await
    }

    async fn full_run(&self) -> Result<ContainerIds, IkkiError> {
        self.ordered_run(self.config.build_order()).await
    }

    async fn stop_all(&self, ids: ContainerIds) -> Result<(), IkkiError> {
        for id in ids {
            docker::remove_container(self.client.clone(), &id).await?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct BuilderHandle {
    sender: CommandSender,
    ids: ContainerIds,
}

impl BuilderHandle {
    pub fn new(client: Docker, config: IkkiConfig) -> Self {
        debug!("setup builder actor");
        let (sender, rx) = mpsc::channel::<Command>(50);
        let builder = Builder::new(rx, client, config);
        task::spawn(run_builder(builder));
        debug!("builder actor setup successful");
        BuilderHandle {
            sender,
            ids: vec![],
        }
    }

    pub async fn run(&self, name: String) -> Result<(), IkkiError> {
        debug!("builder received run request");
        let (response_tx, response_rx) = oneshot::channel();
        let _ = self.sender.send(Command::Run((name, response_tx))).await;
        let run_result = response_rx.await;
        debug!(?run_result, "run result");
        match run_result {
            Err(e) => Err(IkkiError::Other(e.to_string())),
            Ok(RunResult::Error(e)) => Err(e),
            _ => Ok(()),
        }
    }

    pub async fn run_all(&mut self) -> Result<(), IkkiError> {
        debug!("builder received full run request");
        let (response_tx, response_rx) = oneshot::channel();
        let _ = self.sender.send(Command::RunAll(response_tx)).await;
        let run_result = response_rx.await;
        debug!(?run_result, "run all result");
        match run_result {
            Err(e) => Err(IkkiError::Other(e.to_string())),
            Ok(RunResult::Error(e)) => Err(e),
            Ok(RunResult::Success(ids)) => {
                self.ids = ids;
                Ok(())
            }
        }
    }

    pub async fn build(&self, name: String) -> Result<(), IkkiError> {
        debug!("builder received build request");
        let (response_tx, response_rx) = oneshot::channel();
        let _ = self.sender.send(Command::Build((name, response_tx))).await;
        let build_result = response_rx.await;
        debug!(?build_result, "build result");
        match build_result {
            Err(e) => Err(IkkiError::Other(e.to_string())),
            Ok(BuildResult::Error(e)) => Err(e),
            _ => Ok(()),
        }
    }

    pub async fn build_all(&self) -> Result<(), IkkiError> {
        debug!("builder received full build request");
        let (response_tx, response_rx) = oneshot::channel();
        let _ = self.sender.send(Command::BuildAll(response_tx)).await;
        let result = response_rx.await;
        debug!(?result, "build all result");
        match result {
            Err(e) => Err(IkkiError::Other(e.to_string())),
            Ok(BuildResult::Error(e)) => Err(e),
            _ => Ok(()),
        }
    }

    pub async fn stop_all(&self) -> Result<(), IkkiError> {
        debug!("builder received full stop request");
        println!("Stopping and removing all running containers...");
        let (response_tx, response_rx) = oneshot::channel();
        let _ = self
            .sender
            .send(Command::StopAll((self.ids.clone(), response_tx)))
            .await;
        let result = response_rx.await;
        debug!(?result, "stop all result");
        match result {
            Err(e) => Err(IkkiError::Other(e.to_string())),
            Ok(StopResult::Error(e)) => Err(e),
            _ => {
                println!("Successfully stopped and removed all running containers");
                Ok(())
            }
        }
    }
}

async fn run_builder(mut builder: Builder) {
    while let Some(msg) = builder.receiver.recv().await {
        builder.handle_command(msg).await;
    }
    debug!("shutting down builder loop")
}
