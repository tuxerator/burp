use std::f64;
use std::hash::Hash;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::map::Map;
use galileo::Messenger;
use galileo::control::{MouseButton, RawUserEvent};
use galileo::layer::raster_tile_layer::RasterTileLayerBuilder;
use galileo::render::WgpuRenderer;
use galileo_types::cartesian::Point2;
use galileo_types::cartesian::Size;
use galileo_types::geo::Crs;

pub struct EguiMapState<K>
where
    K: Hash + Eq,
{
    map: Map<K>,
    renderer: WgpuRenderer,
    egui_render_state: egui_wgpu::RenderState,
    texture_id: egui::TextureId,
    texture_view: wgpu::TextureView,
    requires_redraw: Arc<AtomicBool>,
}

impl<K> EguiMapState<K>
where
    K: Hash + Eq,
{
    pub fn new(ctx: egui::Context, render_state: egui_wgpu::RenderState, mut map: Map<K>) -> Self {
        let requires_redraw = Arc::new(AtomicBool::new(true));
        let messenger = MapStateMessenger {
            context: ctx.clone(),
            requires_redraw: requires_redraw.clone(),
        };

        let size = Size::new(1, 1);

        {
            let map = map.map_mut();
            map.set_messenger(Some(messenger.clone()));
            map.set_size(size.cast());

            let layers = map.layers_mut();

            layers.iter_mut().for_each(|layer| {
                layer.set_messenger(Box::new(messenger.clone()));
            });
        }

        let renderer = WgpuRenderer::new_with_device_and_texture(
            render_state.device.clone(),
            render_state.queue.clone(),
            size,
        );

        let texture = renderer
            .get_target_texture_view()
            .expect("failed to get map texture");
        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            &texture,
            wgpu::FilterMode::Nearest,
        );

        EguiMapState {
            map,
            renderer,
            egui_render_state: render_state,
            texture_id,
            texture_view: texture,
            requires_redraw,
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui) -> egui::Response {
        log::trace!("[map] Rendering map");
        let available_size = ui.max_rect().size();
        let map_size = self.renderer.size().cast::<f32>();

        let (rect, response) =
            ui.allocate_exact_size(available_size, egui::Sense::click_and_drag());

        if response.contains_pointer() {
            let events = ui.input(|input| input.events.clone());
            self.process_events(&events, [-rect.left(), -rect.top()]);
        }

        self.map.map_mut().animate();

        if available_size[0] != map_size.width() || available_size[1] != map_size.height() {
            self.resize_map(available_size);
        }

        if self.requires_redraw.swap(false, Ordering::Relaxed) {
            self.draw();
        }

        egui::Image::new(egui::ImageSource::Texture(egui::load::SizedTexture::new(
            self.texture_id,
            egui::Vec2::new(map_size.width(), map_size.height()),
        )))
        .paint_at(ui, rect);

        response
    }

    fn resize_map(&mut self, size: egui::Vec2) {
        log::trace!("Resizing map to size: {size:?}");

        let size = Size::new(size.x as f64, size.y as f64);
        self.map.map_mut().set_size(size);

        let size = Size::new(size.width() as u32, size.height() as u32);
        self.renderer.resize(size);

        // After renderer is resized, a new texture is created, so we need to update its id that we
        // use in UI.
        let texture = self
            .renderer
            .get_target_texture_view()
            .expect("failed to get map texture");
        let texture_id = self
            .egui_render_state
            .renderer
            .write()
            .register_native_texture(
                &self.egui_render_state.device,
                &texture,
                wgpu::FilterMode::Nearest,
            );

        self.texture_id = texture_id;
        self.texture_view = texture;

        self.map.map().redraw();
    }

    fn draw(&mut self) {
        log::trace!("[map] Redrawing the map");
        self.map.map().load_layers();
        self.renderer
            .render_to_texture_view(self.map.map(), &self.texture_view);
    }

    fn process_events(&mut self, events: &[egui::Event], offset: [f32; 2]) {
        for event in events {
            if let Some(raw_event) = Self::convert_event(event, offset) {
                self.map.handle_event(raw_event);
            }
        }
    }

    fn convert_event(event: &egui::Event, offset: [f32; 2]) -> Option<RawUserEvent> {
        match event {
            egui::Event::PointerButton {
                button, pressed, ..
            } => {
                let button = match button {
                    egui::PointerButton::Primary => MouseButton::Left,
                    egui::PointerButton::Secondary => MouseButton::Right,
                    egui::PointerButton::Middle => MouseButton::Middle,
                    _ => MouseButton::Other,
                };

                Some(match pressed {
                    true => RawUserEvent::ButtonPressed(button),
                    false => RawUserEvent::ButtonReleased(button),
                })
            }
            egui::Event::PointerMoved(position) => {
                let scale = 1.0;
                let pointer_position = Point2::new(
                    (position.x + offset[0]) as f64 / scale,
                    (position.y + offset[1]) as f64 / scale,
                );
                Some(RawUserEvent::PointerMoved(pointer_position))
            }
            #[cfg(not(target_arch = "wasm32"))]
            egui::Event::MouseWheel { delta, .. } => {
                let zoom = delta[1] as f64;

                if zoom.abs() < 0.0001 {
                    return None;
                }

                Some(RawUserEvent::Scroll(zoom))
            }
            #[cfg(target_arch = "wasm32")]
            Event::MouseWheel { delta, unit, .. } => {
                // Winit produces different values in different browsers and they are all different
                // from native platforms. See ttps://github.com/rust-windowing/winit/issues/22
                //
                // This hack is based on manual tests and might break in future. But this is the
                // best I could come up with to mitigate the issue.
                let zoom = match unit {
                    egui::MouseWheelUnit::Point => delta[1] as f64 / 120.0,
                    egui::MouseWheelUnit::Line => delta[1] as f64 / 6.0,
                    egui::MouseWheelUnit::Page => delta[1] as f64,
                };

                if zoom.abs() < 0.0001 {
                    return None;
                }

                Some(RawUserEvent::Scroll(zoom))
            }

            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MapStateMessenger {
    pub requires_redraw: Arc<AtomicBool>,
    pub context: egui::Context,
}

impl Messenger for MapStateMessenger {
    fn request_redraw(&self) {
        log::trace!("Redraw requested");
        if !self.requires_redraw.swap(true, Ordering::Relaxed) {
            self.context.request_repaint();
        }
    }
}
