use std::sync::Arc;

use ashpd::WindowIdentifier;
use egui::Widget;
use parking_lot::{Mutex, RawMutex};
use tokio::{
    runtime::Runtime,
    task::{JoinHandle, LocalSet},
};
use wgpu::rwh::{HasDisplayHandle, HasWindowHandle};

pub struct OpenFile<'a, 'b> {
    label: String,
    frame: &'a eframe::Frame,
    runtime: &'b Runtime,
    callback: Box<dyn Fn(&std::path::Path) + Send>,
}

impl<'a, 'b> OpenFile<'a, 'b> {
    pub fn new(
        label: impl Into<String>,
        frame: &'a eframe::Frame,
        runtime: &'b Runtime,
        callback: impl Fn(&std::path::Path) + Send + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            frame,
            runtime,
            callback: Box::new(callback),
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
                        log::debug!("[xdg-desktop-portal] WindowIdentifier {:?}", identifier);
                        let files = ashpd::desktop::file_chooser::OpenFileRequest::default()
                            .identifier(identifier)
                            .multiple(false)
                            .send()
                            .await
                            .unwrap()
                            .response()
                            .unwrap();

                        let urls = files.uris();
                        let url = urls.first().unwrap();
                        (self.callback)(std::path::Path::new(url.path()));
                    })))
                });
            });
        }

        button
    }
}
