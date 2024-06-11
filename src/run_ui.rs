extern crate geozero;

use std::fs::File;
use std::io::{BufReader, Read};

use egui::{Context, Id};
use galileo_types::geo::impls::GeoPoint2d;
use galileo_types::geo::GeoPoint;
use geo::Coord;
use graph_rs::graph::csr::DirectedCsrGraph;
use graph_rs::input::geo_zero::geozero::geojson::read_geojson;
use graph_rs::input::geo_zero::GraphWriter;
use graph_rs::{DirectedGraph, Graph};
use ordered_float::OrderedFloat;
use rfd::FileDialog;

#[derive(Debug)]
pub struct UiState {
    pub positions: Positions,
    pub map_hidden: bool,
    pub graph: Option<DirectedCsrGraph<OrderedFloat<f64>, Coord<OrderedFloat<f64>>>>,
}

impl UiState {
    pub fn new() -> Self {
        Self {
            positions: Positions::default(),
            map_hidden: false,
            graph: None,
        }
    }
}

#[derive(Clone, Default, Debug)]
pub struct Positions {
    pub pointer_position: Option<GeoPoint2d>,
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
            let file_path = FileDialog::new().set_directory("~/").pick_file().unwrap();

            let file = File::open(file_path).unwrap();
            let mut buf_reader = BufReader::new(file);
            let mut geojson = String::new();

            buf_reader.read_to_string(&mut geojson);

            let mut graph_writer = GraphWriter::default();
            graph_writer.filter_features();

            read_geojson(geojson.as_bytes(), &mut graph_writer);

            state.graph = Some(graph_writer.get_graph());
        }
    });
}
