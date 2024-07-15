extern crate geozero;

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};

use egui::{Context, Id};
use egui_graphs::Node;
use galileo_types::geo::impls::GeoPoint2d;
use galileo_types::geo::GeoPoint;
use geo::{Area, Coord};
use geozero::geojson::read_geojson;
use graph_rs::graph::csr::DirectedCsrGraph;
use graph_rs::graph::quad_tree::QuadGraph;
use graph_rs::{CoordGraph, DirectedGraph, Graph};
use log::info;
use ordered_float::OrderedFloat;
use rfd::FileDialog;

use crate::input::geo_zero::{ColumnValueClonable, GraphWriter, PoiWriter};
use crate::input::NodeValue;
use crate::state::Events;

pub struct UiState {
    pub positions: Positions,
    pub graph: Arc<RwLock<Option<QuadGraph<f64, NodeValue, DirectedCsrGraph<f64, NodeValue>>>>>,
    pub sender: Sender<Events>,
}

impl UiState {
    pub fn new(
        graph: Arc<RwLock<Option<QuadGraph<f64, NodeValue, DirectedCsrGraph<f64, NodeValue>>>>>,
        sender: Sender<Events>,
    ) -> Self {
        Self {
            positions: Positions::default(),
            graph,
            sender,
        }
    }
}

#[derive(Clone, Default, Debug)]
pub struct Positions {
    pub pointer_position: Option<GeoPoint2d>,
    pub click_position: Option<GeoPoint2d>,
    pub map_center_position: Option<GeoPoint2d>,
}

pub fn run_ui(state: &mut UiState, ctx: &Context) {
    egui::Window::new("Galileo map").show(ctx, |ui| {
        ui.label("Pointer position:");
        if let Some(pointer_position) = state.positions.pointer_position {
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
        if let Some(map_center_position) = state.positions.map_center_position {
            ui.label(format!(
                "Lat: {:.4} Lon: {:.4}",
                map_center_position.lat(),
                map_center_position.lon()
            ));
        } else {
            ui.label("<unavaliable>");
        }
    });

    egui::SidePanel::right("Left panel").show(ctx, |ui| {
        if ui.add(egui::Button::new("Load graph")).clicked() {
            let graph_ref = Arc::clone(&state.graph);
            let sender_clone = state.sender.clone();
            let file_path = FileDialog::new().set_directory("~/").pick_file().unwrap();

            tokio::spawn(async move {
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
                let mut graph_writer = GraphWriter::new(filter);

                read_geojson(buf_reader, &mut graph_writer);
                let mut graph = graph_ref.write().expect("poisoned lock");
                *graph = Some(QuadGraph::new_from_graph(graph_writer.get_graph()));
                sender_clone
                    .send(Events::BuildGraphLayer)
                    .expect("reciever was deallocated");
            });
        }

        if ui.add(egui::Button::new("Load POIs")).clicked() {
            let sender_clone = state.sender.clone();
            let file_path = FileDialog::new().set_directory("~/").pick_file().unwrap();
            let graph_ref = Arc::clone(&state.graph);

            tokio::spawn(async move {
                let file = File::open(file_path).unwrap();
                let buf_reader = BufReader::new(file);

                let mut poi_writer = PoiWriter::new(|_| true, graph_ref);
                read_geojson(buf_reader, &mut poi_writer);
                info!("Loaded Pois");
            });
        }
    });
}
