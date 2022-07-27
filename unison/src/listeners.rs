use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use bollard::Docker;
use futures::prelude::*;
use notify::DebouncedEvent;
use notify::Watcher;
use notify::{watcher, RecursiveMode};

use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task;
use tokio::task::JoinHandle;
use tracing::debug;
use unison_config::Image;

use crate::supervisor::Event;
use crate::supervisor::EventSender;

use crate::supervisor::ImageSourceLocations;
use crate::UnisonError;

struct FsEventListener {
    shutdown: oneshot::Receiver<()>,
    event_sender: EventSender,
    image_source_locations: ImageSourceLocations,
}

impl FsEventListener {
    fn new(
        event_sender: EventSender,
        shutdown: oneshot::Receiver<()>,
        image_source_locations: ImageSourceLocations,
    ) -> Self {
        Self {
            event_sender,
            shutdown,
            image_source_locations,
        }
    }
}

pub struct FsEventListenerHandle {
    sender: oneshot::Sender<()>,
    handle: JoinHandle<Result<(), UnisonError>>,
}

impl FsEventListenerHandle {
    pub fn new(image_source_locations: ImageSourceLocations, event_sender: EventSender) -> Self {
        debug!("setup FS event listener");
        let (sender, rx) = oneshot::channel();
        let listener = FsEventListener::new(event_sender, rx, image_source_locations);
        let handle = task::spawn(run_fs_event_listener(listener));
        debug!("FS event listener setup successful");
        Self { sender, handle }
    }

    pub async fn shutdown(self) {
        let _ = self.sender.send(());
        let _ = self.handle.await;
    }
}

async fn run_fs_event_listener(mut listener: FsEventListener) -> Result<(), UnisonError> {
    let (fs_event_sender, mut fs_event_receiver) = mpsc::channel(10);
    let (watcher_sender, blocking_fs_receiver) = std::sync::mpsc::channel();

    debug!("starting FS event watcher");

    let mut watcher =
        watcher(watcher_sender, Duration::from_secs(2)).map_err(|_| UnisonError::FileWatcher)?;

    for path in listener.image_source_locations.keys() {
        watcher
            .watch(path, RecursiveMode::Recursive)
            .map_err(|_| UnisonError::FileWatcher)?;
    }

    let fs_watcher =
        task::spawn_blocking(move || watch_file_changes(blocking_fs_receiver, fs_event_sender));

    loop {
        tokio::select! {
                _ = &mut listener.shutdown => {
                    drop(watcher);
                    let _ = fs_watcher.await;
                    debug!("FS event listener thread successfully shutdown");
                    return Ok(())
                },
                Some(event) = fs_event_receiver.recv() => {
                    // Ignore everything that is not a create/write/remove event
                    match event {
                        DebouncedEvent::Create(path)
                        | DebouncedEvent::Write(path)
                        | DebouncedEvent::Remove(path) => {
                            let canonical_path = path.parent().and_then(|p| p.canonicalize().ok()).unwrap();
                            if let Some(image_name) = listener.image_source_locations.get(&canonical_path) {
                                listener.event_sender.send(Event::SourceChanged(image_name.clone())).await
                                    .map_err(|_| UnisonError::FileWatcher)?
                            }
                        }
                        _ => ()
                }
            }
        }
    }
}

fn watch_file_changes(
    receiver: std::sync::mpsc::Receiver<DebouncedEvent>,
    sender: mpsc::Sender<DebouncedEvent>,
) -> Result<(), UnisonError> {
    while let Ok(event) = receiver.recv() {
        debug!(?event, "detected filesystem change");
        sender
            .blocking_send(event)
            .map_err(|_| UnisonError::FileWatcher)?;
    }

    Ok(())
}
