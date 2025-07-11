use std::sync::Arc;

use ashpd::WindowIdentifier;
use burp::{oracle::PoiGraph, types::Poi};
use eframe::CreationContext;
use egui::{Frame, FrameDurations, Modal, widget_text};
use parking_lot::Mutex;
use tokio::task::LocalSet;
use wgpu::rwh::{HasDisplayHandle, HasWindowHandle};

use crate::{
    map::{Map, egui_state::EguiMapState},
    widgets,
};

pub struct BurpApp {
    map: EguiMapState<String>,
    runtime: tokio::runtime::Runtime,
    graph: Option<PoiGraph<Poi>>,
}

impl BurpApp {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        let map_state = EguiMapState::new(
            cc.egui_ctx.clone(),
            cc.wgpu_render_state
                .clone()
                .expect("failed to get wgpu context"),
            Map::default(),
        );

        let runtime = tokio::runtime::Runtime::new().unwrap();

        Self {
            map: map_state,
            runtime,
            graph: None,
        }
    }
}

impl eframe::App for BurpApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let _rt_ctx = self.runtime.enter();
        egui::SidePanel::right("Right Panel").show(ctx, |ui| {
            ui.add(widgets::OpenFile::new(
                "Open File",
                frame,
                &self.runtime,
                |path| log::info!("Opened file {path:?}"),
            ));

            // if ui.add(egui::Button::new("Load graph")).clicked() {
            //     // let file = File::open(file_path).unwrap();
            //     // let buf_reader = BufReader::new(file);
            //     //
            //     // let filter = |p: &HashMap<String, ColumnValueClonable>| {
            //     //     let footway = p.get("footway");
            //     //     let highway = p.get("highway");
            //     //
            //     //     match highway {
            //     //         None => return false,
            //     //         Some(ColumnValueClonable::String(s)) if s == "null" => return false,
            //     //         Some(ColumnValueClonable::String(s)) if s == "cycleway" => return false,
            //     //         Some(ColumnValueClonable::String(s)) if s == "path" => return false,
            //     //         Some(ColumnValueClonable::String(s)) if s == "footway" => return false,
            //     //         Some(ColumnValueClonable::String(s)) if s == "steps" => return false,
            //     //         Some(ColumnValueClonable::String(s)) if s == "corridor" => return false,
            //     //         _ => (),
            //     //     }
            //     //
            //     //     match footway {
            //     //         None => true,
            //     //         Some(ColumnValueClonable::String(s)) => s == "null",
            //     //         _ => false,
            //     //     }
            //     // };
            //     // let mut graph_writer = GraphWriter::new(filter);
            //     //
            //     // read_geojson(buf_reader, &mut graph_writer);
            //     // let graph = RTreeGraph::new_from_graph(graph_writer.get_graph());
            //     // state.graph = Some(PoiGraph::new(graph));
            //     //
            //     // state.state = State::LoadedGraph;
            // }
        });
        egui::CentralPanel::default()
            .frame(Frame::new().inner_margin(0).outer_margin(0))
            .show(ctx, |ui| {
                self.map.render(ui);
            });
    }
}
