extern crate geozero;

use std::fs::File;
use std::io::{BufReader, Read};
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};

use egui::{Context, Id};
use galileo_types::geo::impls::GeoPoint2d;
use galileo_types::geo::GeoPoint;
use geo::Coord;
use graph_rs::graph::csr::DirectedCsrGraph;
use graph_rs::graph::quad_tree::QuadGraph;
use graph_rs::input::geo_zero::geozero::geojson::read_geojson;
use graph_rs::input::geo_zero::GraphWriter;
use graph_rs::{DirectedGraph, Graph};
use ordered_float::OrderedFloat;
use rfd::FileDialog;

use crate::state::Events;

pub struct UiState {
    pub positions: Positions,
    pub graph: Arc<RwLock<Option<QuadGraph<f64, DirectedCsrGraph<f64, Coord<f64>>>>>>,
    pub sender: Sender<Events>,
}

impl UiState {
    pub fn new(
        graph: Arc<RwLock<Option<QuadGraph<f64, DirectedCsrGraph<f64, Coord<f64>>>>>>,
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
        if ui.add(egui::Button::new("Load")).clicked() {
            let graph_ref = Arc::clone(&state.graph);
            let sender_clone = state.sender.clone();
            let file_path = FileDialog::new().set_directory("~/").pick_file().unwrap();

            tokio::spawn(async move {
                let file = File::open(file_path).unwrap();
                let buf_reader = BufReader::new(file);

                let mut graph_writer = GraphWriter::default();
                graph_writer.filter_features();

                read_geojson(buf_reader, &mut graph_writer);
                let mut graph = graph_ref.write().expect("poisoned lock");
                *graph = Some(QuadGraph::new_from_graph(graph_writer.get_graph()));
                sender_clone
                    .send(Events::BuildGraphLayer)
                    .expect("reciever was deallocated");
            });
        }
    });
}
