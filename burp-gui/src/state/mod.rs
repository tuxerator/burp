use std::{
    collections::HashMap,
    fs::File,
    io::BufReader,
    iter,
    path::PathBuf,
    sync::{
        mpsc::{self, Receiver},
        Arc, RwLock,
    },
};

// use crate::run_ui::{run_ui, UiState};

use burp::{
    galileo::GalileoMap,
    input::geo_zero::{ColumnValueClonable, GraphWriter},
    oracle::Oracle,
    types::Poi,
};
use geo::Coord;
use geozero::geojson::read_geojson;
use graph_rs::graph::{csr::DirectedCsrGraph, quad_tree::QuadGraph};
use ui_state::Ui;
use wgpu::TextureView;
use winit::{event::*, window::Window};

use self::{egui_state::EguiState, galileo_state::GalileoState};

mod egui_state;
mod galileo_state;
mod ui_state;

pub struct WgpuFrame<'frame> {
    device: &'frame wgpu::Device,
    queue: &'frame wgpu::Queue,
    encoder: &'frame mut wgpu::CommandEncoder,
    window: &'frame Window,
    texture_view: &'frame TextureView,
    size: winit::dpi::PhysicalSize<u32>,
}

pub enum Events {
    BuildGraphLayer,
    LoadGraphFromPath(PathBuf),
}

pub struct State {
    pub surface: Arc<wgpu::Surface<'static>>,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub window: Arc<Window>,
    pub egui_state: EguiState,
    pub galileo_state: GalileoState,
    pub ui_state: Ui,
    pub oracle: Arc<RwLock<Option<Oracle<Poi>>>>,
    pub reciever: Receiver<Events>,
}

impl State {
    pub async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits {
                            // NOTE(alexkirsz) These are the limits on my GPU w/ WebGPU,
                            // but your mileage may vary.
                            max_texture_dimension_2d: 16384,
                            ..wgpu::Limits::downlevel_webgl2_defaults()
                        }
                    } else {
                        wgpu::Limits::default()
                    },
                },
                None,
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            desired_maximum_frame_latency: 2,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let egui_state = EguiState::new(&device, config.format, None, 1, &window);

        let surface = Arc::new(surface);
        let device = Arc::new(device);
        let queue = Arc::new(queue);
        let graph = Arc::new(RwLock::new(None));

        let (sender, reciever) = mpsc::channel();

        let galileo_state = GalileoState::new(
            Arc::clone(&window),
            Arc::clone(&device),
            Arc::clone(&surface),
            Arc::clone(&queue),
            config.clone(),
            Arc::clone(&graph),
        );

        let positions = galileo_state.positions();

        let ui_state = Ui::new(galileo_state.map(), positions.pointer_pos);

        Self {
            surface,
            device,
            queue,
            config,
            size,
            window,
            egui_state,
            galileo_state,
            ui_state,
            oracle: graph,
            reciever,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn about_to_wait(&mut self) {
        self.galileo_state.about_to_wait();
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.galileo_state.resize(new_size);
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn handle_event(&mut self, event: &WindowEvent) {
        let res = self.egui_state.handle_event(&self.window, event);

        if !res.consumed {
            self.galileo_state.handle_event(event);
        }

        self.window().request_redraw();
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        if let Ok(event) = self.reciever.try_recv() {
            self.process_event(event);
        }

        let texture = self.surface.get_current_texture()?;

        let texture_view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: None,
            dimension: None,
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut wgpu_frame = WgpuFrame {
                device: &self.device,
                queue: &self.queue,
                encoder: &mut encoder,
                window: &self.window,
                texture_view: &texture_view,
                size: self.size,
            };

            self.galileo_state.render(&wgpu_frame);

            // self.egui_state.render(&mut wgpu_frame, |ctx| {
            //     self.ui_state.run_ui(ctx);
            // });
        }

        self.queue.submit(iter::once(encoder.finish()));

        texture.present();

        Ok(())
    }

    fn process_event(&self, event: Events) {
        match event {
            Events::BuildGraphLayer => self.galileo_state.build_map_layer(),
            Events::LoadGraphFromPath(path) => self.load_graph_from_path(path),
            _ => (),
        };
    }

    fn load_graph_from_path(&self, path: PathBuf) {
        let oracle_ref = Arc::clone(&self.oracle);
        let map = self.galileo_state.map();

        tokio::spawn(async move {
            let file = File::open(path).unwrap();
            let buf_reader = BufReader::new(file);

            let filter = |p: &HashMap<String, ColumnValueClonable>| {
                let footway = p.get("footway");
                let highway = p.get("highway");

                match highway {
                    None => return false,
                    Some(ColumnValueClonable::String(s)) if s == "null" => return false,
                    _ => (),
                }

                match footway {
                    None => true,
                    Some(ColumnValueClonable::String(s)) => s == "null",
                    _ => false,
                }
            };
            let galileo_map = GalileoMap::new(map.clone());
            let mut graph_writer = GraphWriter::new(filter, Some(galileo_map));

            read_geojson(buf_reader, &mut graph_writer);
            let mut oracle = oracle_ref.write().expect("poisoned lock");
            let graph = QuadGraph::new_from_graph(graph_writer.get_graph());
            *oracle = Some(Oracle::new(graph, map));
        });
    }
}
