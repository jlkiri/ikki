

use std::collections::HashMap;
use std::path::{PathBuf};
use tokio::sync::mpsc;
use tokio::task;
use tokio::task::JoinHandle;
use tracing::debug;

use crate::builder::{BuilderHandle};
use crate::listeners::FsEventListenerHandle;
use crate::UnisonError;

type ImageName = String;
pub type ImageSourceLocations = HashMap<PathBuf, ImageName>;

#[derive(Debug)]
pub enum Event {
    SourceChanged(ImageName),
    Shutdown,
}

pub type EventReceiver = mpsc::Receiver<Event>;
pub type EventSender = mpsc::Sender<Event>;

pub struct Supervisor {
    builder_handle: BuilderHandle,
    receiver: EventReceiver,
    image_source_locations: ImageSourceLocations,
}

impl Supervisor {
    fn new(
        builder: BuilderHandle,
        image_source_locations: ImageSourceLocations,
        receiver: EventReceiver,
    ) -> Self {
        Self {
            builder_handle: builder,
            receiver,
            image_source_locations,
        }
    }
}

#[derive(Debug)]
pub enum Mode {
    BuildOnly,
    Run,
}

pub struct SupervisorHandle {
    sender: EventSender,
    fs_event_handle: FsEventListenerHandle,
    handle: JoinHandle<()>,
}

impl SupervisorHandle {
    pub fn new(
        image_source_locations: ImageSourceLocations,
        builder: BuilderHandle,
        mode: Mode,
    ) -> Self {
        let (sender, rx) = mpsc::channel::<Event>(10);
        let supervisor = Supervisor::new(builder, image_source_locations.clone(), rx);
        let handle = task::spawn(run_supervisor(supervisor, mode));
        let fs_event_handle = FsEventListenerHandle::new(image_source_locations, sender.clone());

        Self {
            sender,
            handle,
            fs_event_handle,
        }
    }

    pub async fn shutdown(self) -> Result<(), UnisonError> {
        let _ = self.sender.send(Event::Shutdown).await;

        drop(self.sender);

        debug!("shutting down fs event listener...");
        self.fs_event_handle.shutdown().await;

        debug!("shutting down supervisor loop...");
        self.handle
            .await
            .map_err(|e| UnisonError::Other(e.to_string()))?;
        Ok(())
    }
}

async fn run_supervisor(mut supervisor: Supervisor, mode: Mode) {
    while let Some(msg) = supervisor.receiver.recv().await {
        match msg {
            Event::Shutdown => {
                if let Err(e) = supervisor.builder_handle.stop_all().await {
                    println!("Unison error: {}", e)
                }
            }
            Event::SourceChanged(image_name) => {
                if let Err(e) = supervisor.builder_handle.build(image_name.clone()).await {
                    println!("Unison error: {}", e)
                }

                if let Mode::Run = mode {
                    if let Err(e) = supervisor.builder_handle.run(image_name).await {
                        println!("Unison error: {}", e)
                    }
                }
            }
        }
    }
}
