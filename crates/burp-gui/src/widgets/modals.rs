use std::error::Error;

use egui_modal::Modal;

pub struct ErrorModal {
    modal: egui_modal::Modal,
}

impl ErrorModal {
    pub fn new(ctx: &egui::Context, id: impl std::fmt::Display) -> Self {
        Self {
            modal: egui_modal::Modal::new(ctx, id),
        }
    }

    pub fn show(&mut self) {
        self.modal.show_dialog();
    }

    pub fn handle_error<T>(
        &self,
        ui: &mut egui::Ui,
        contents: impl FnOnce(&mut egui::Ui) -> Result<T, Box<dyn Error>>,
    ) -> Option<T> {
        let result = contents(ui);

        match result {
            Ok(response) => Some(response),
            Err(err) => {
                self.modal
                    .dialog()
                    .with_title("Error")
                    .with_icon(egui_modal::Icon::Error)
                    .with_body(err)
                    .open();
                None
            }
        }
    }
}
