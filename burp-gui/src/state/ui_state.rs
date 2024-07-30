use std::{
    collections::HashMap,
    fs::File,
    future::Future,
    io::BufReader,
    mem,
    sync::{Arc, RwLock},
};

use burp::{
    input::geo_zero::{ColumnValueClonable, GraphWriter, PoiWriter},
    oracle::Oracle,
    types::Poi,
};
use egui::{Context, InnerResponse, SidePanel, Ui as EguiUi};
use galileo::Map;
use galileo_types::geo::{impls::GeoPoint2d, GeoPoint};
use geozero::geojson::read_geojson;
use graph_rs::graph::quad_tree::QuadGraph;
use petgraph::EdgeType;
use rfd::FileDialog;
use tokio::{
    runtime::Handle,
    task::{block_in_place, JoinHandle},
};

use crate::types::PointerPos;

use super::{galileo_state::GalileoState, Events};

trait RunnableUi {
    fn side_panel(&mut self, ctx: &Context);
}

pub struct Ui {
    map: Arc<RwLock<Map>>,
    pointer_pos: Arc<RwLock<PointerPos>>,
    load_oracle: Option<JoinHandle<Oracle<Poi>>>,
    oracle: Option<Oracle<Poi>>,
}

impl Ui {
    pub fn new(map: Arc<RwLock<Map>>, pointer_pos: Arc<RwLock<PointerPos>>) -> Self {
        Self {
            map,
            pointer_pos,
            load_oracle: None,
            oracle: None,
        }
    }

    pub fn run_ui(&mut self, ctx: &Context) {
        self.map_window(ctx);
        if self.oracle.is_none() {
            self.init(ctx);
            return;
        }
    }

    fn init(&mut self, ctx: &Context) {
        self.load_oracle = if let Some(join_handle) = self.load_oracle.take() {
            if join_handle.is_finished() {
                let oracle = Handle::current().block_on(join_handle);
                if let Ok(oracle) = oracle {
                    self.oracle = Some(oracle);
                }
                None
            } else {
                Some(join_handle)
            }
        } else {
            egui::SidePanel::right("right_panel").show(ctx, |ui| {
                if ui.button("Load Graph").clicked() {
                    self.load_oracle = Some(Ui::load_oracle());
                }
            });
            None
        }
    }

    fn map_window(&self, ctx: &Context) {
        egui::Window::new("Galileo map").show(ctx, |ui| {
            ui.label("Pointer position:");
            if let Some(pointer_position) =
                self.pointer_pos.read().expect("poisoned lock").geo_pos()
            {
                ui.label(format!(
                    "Lat: {:.4} Lon: {:.4}",
                    pointer_position.lat(),
                    pointer_position.lon()
                ));
            } else {
                ui.label("<unavaliable>");
            }

            ui.separator();

            ui.label("Map center position:");
            if let Some(map_center_position) =
                self.map.read().expect("poisoned lock").view().position()
            {
                ui.label(format!(
                    "Lat: {:.4} Lon: {:.4}",
                    map_center_position.lat(),
                    map_center_position.lon()
                ));
            } else {
                ui.label("<unavaliable>");
            }
        });
    }

    fn load_oracle() -> JoinHandle<Oracle<Poi>> {
        let file_path = FileDialog::new().set_directory("~/").pick_file().unwrap();

        let joind_handle = tokio::spawn(async move {
            let file = File::open(file_path).unwrap();
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
            let mut graph_writer = GraphWriter::new(filter, None);

            read_geojson(buf_reader, &mut graph_writer);
            let graph = QuadGraph::new_from_graph(graph_writer.get_graph());
            Oracle::from(graph)
        });
        joind_handle
    }
}

enum State {
    Init,
    LoadOracle,
    Default,
    Dijkstra,
}

#[derive(Debug, Default)]
pub struct Init {
    load_graph: bool,
}

impl RunnableUi for Init {
    fn side_panel(&mut self, ctx: &Context) {
        egui::SidePanel::right("right_panel").show(ctx, |ui| {
            self.load_graph = ui.button("Load Graph").clicked();
        });
    }
}

impl Init {
    fn load_oracle(&self) -> LoadGraph {
        let file_path = FileDialog::new().set_directory("~/").pick_file().unwrap();

        let joind_handle = tokio::spawn(async move {
            let file = File::open(file_path).unwrap();
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
            let mut graph_writer = GraphWriter::new(filter, None);

            read_geojson(buf_reader, &mut graph_writer);
            let graph = QuadGraph::new_from_graph(graph_writer.get_graph());
            Oracle::from(graph)
        });
        LoadGraph {
            oracle_join_hanle: joind_handle,
        }
    }
}

struct LoadGraph {
    oracle_join_hanle: JoinHandle<Oracle<Poi>>,
}

struct DefaultState {
    oracle: Oracle<Poi>,
    loaded_pois: bool,
}

impl RunnableUi for DefaultState {
    fn side_panel(&mut self, ctx: &Context) {
        egui::SidePanel::right("right_panel").show(ctx, |ui| {
            self.loaded_pois = ui.button("Load Pois").clicked();
        });
    }
}
