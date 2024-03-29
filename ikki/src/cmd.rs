use bollard::Docker;
use ikki_config::IkkiConfig;
use miette::IntoDiagnostic;
use tokio::signal;
use tracing::debug;

use crate::{
    builder::BuilderHandle,
    docker::DockerError,
    docker_config::*,
    supervisor::{ImageSourceLocations, Mode, SupervisorHandle},
};

pub async fn explain(config: IkkiConfig) -> miette::Result<()> {
    let build_options = config
        .images()
        .iter()
        .map(build_options)
        .collect::<Result<Vec<BuildOptions>, DockerError>>()
        .into_diagnostic()?;

    let cmds = build_options.into_iter().map(|opt| opt.explain());

    for cmd in cmds {
        println!("{cmd}");
    }

    let run_options = config
        .images()
        .iter()
        .cloned()
        .filter(|img| img.service.is_some())
        .map(|img| {
            (
                img.name.clone(),
                img.pull.unwrap_or(img.name),
                img.service.unwrap(),
            )
        })
        .map(create_run_options)
        .collect::<Vec<RunOptions>>();

    let cmds = run_options.into_iter().map(|opt| opt.explain());

    for cmd in cmds {
        println!("{cmd}");
    }

    Ok(())
}

pub async fn up(config: IkkiConfig) -> miette::Result<()> {
    let docker = Docker::connect_with_local_defaults().into_diagnostic()?;

    debug!("connected to docker daemon");

    println!("Calculated image build order:");
    println!();

    for (i, chunk) in config.build_order().iter().enumerate() {
        println!("[{}] {}", i + 1, chunk.join(", "));
    }
    println!();

    let image_source_locations: ImageSourceLocations = config
        .images()
        .iter()
        .filter_map(|img| {
            img.path
                .clone()
                .map(|path| (path.canonicalize().unwrap(), img.name.clone()))
        })
        .collect();

    let mut builder = BuilderHandle::new(docker.clone(), config);

    builder.build_all().await?;
    builder.run_all().await?;

    let supervisor = SupervisorHandle::new(image_source_locations, builder, Mode::Run);

    println!("Watching for source changes...");

    match signal::ctrl_c().await {
        Ok(()) => {
            debug!("received SIGINT signal, shutting down...");

            supervisor
                .shutdown()
                .await
                .expect("failed to gracefully shutdown the supervisor")
        }
        Err(err) => {
            eprintln!("unable to listen for shutdown signal: {}", err);
        }
    }

    debug!("all shutdown");
    Ok(())
}
