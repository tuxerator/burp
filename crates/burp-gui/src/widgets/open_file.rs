use std::sync::Arc;

use ashpd::{WindowIdentifier, desktop::file_chooser::FileFilter};
use egui::Widget;
use parking_lot::{Mutex, RawMutex};
use tokio::{
    runtime::Runtime,
    sync::mpsc::Sender,
    task::{JoinHandle, LocalSet},
};
use wgpu::rwh::{HasDisplayHandle, HasWindowHandle};

use crate::event_handler::Event;

pub struct OpenFile<'a, 'b> {
    label: String,
    file_filter: Vec<FileFilter>,
    frame: &'a eframe::Frame,
    runtime: &'b Runtime,
    callback: Box<dyn Fn(&std::path::Path) -> Option<Event> + Send>,
    sender: Sender<Event>,
}

impl<'a, 'b> OpenFile<'a, 'b> {
    pub fn new(
        label: impl Into<String>,
        file_filter: Vec<FileFilter>,
        frame: &'a eframe::Frame,
        runtime: &'b Runtime,
        sender: Sender<Event>,
        callback: impl Fn(&std::path::Path) -> Option<Event> + Send + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            file_filter,
            frame,
            runtime,
            callback: Box::new(callback),
            sender,
        }
    }
}

impl Widget for OpenFile<'_, '_> {
    #[cfg(target_os = "linux")]
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let button = ui.button(self.label);

        if button.clicked() {
            ui.memory_mut(|mem| {
                if let Some(handle) = mem.data.get_temp::<Arc<Mutex<JoinHandle<()>>>>(button.id) {
                    let handle = handle.lock();
                    if handle.is_finished() {
                        mem.data.remove::<Arc<Mutex<JoinHandle<()>>>>(button.id);
                        log::debug!("Removed file dialog handle");
                    } else {
                        log::info!("File Dialog already open");
                        return;
                    }
                }
                mem.data.get_temp_mut_or_insert_with(button.id, move || {
                    let identifier = LocalSet::new().block_on(&self.runtime, async {
                        WindowIdentifier::from_raw_handle(
                            &self.frame.window_handle().unwrap().as_raw(),
                            Some(&self.frame.display_handle().unwrap().as_raw()),
                        )
                        .await
                    });
                    Arc::new(Mutex::new(self.runtime.spawn(async move {
                        use ashpd::desktop::file_chooser::FileFilter;

                        log::debug!("[xdg-desktop-portal] WindowIdentifier {:?}", identifier);
                        let files = ashpd::desktop::file_chooser::OpenFileRequest::default()
                            .identifier(identifier)
                            .multiple(false)
                            .filters(self.file_filter)
                            .send()
                            .await
                            .unwrap()
                            .response()
                            .unwrap();

                        if let Some(event) = tokio::task::spawn_blocking(move || {
                            let urls = files.uris();
                            let url = urls.first().unwrap();
                            (self.callback)(std::path::Path::new(url.path()))
                        })
                        .await
                        .expect("[OpenFile] Callback panic")
                        {
                            self.sender
                                .send(event)
                                .await
                                .expect("[OpenFile] Failed to send event");
                            tracing::debug!("Event \x1b[1mGraphLoaded\x1b[0m send")
                        }
                    })))
                });
            });
        }

        button
    }
}
