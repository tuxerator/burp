extern crate geozero;

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};

use burp::oracle::Oracle;
use burp::types::Poi;
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

use crate::state::Events;
use crate::types::MapPositions;
use burp::input::geo_zero::{ColumnValueClonable, GraphWriter, PoiWriter};

pub struct UiState {
    positions: Arc<RwLock<MapPositions>>,
    oracle: Arc<RwLock<Option<Oracle<Poi>>>>,
    sender: Sender<Events>,
    state: State,
}

#[derive(PartialEq)]
enum State {
    Init,
    LoadedGraph,
    LoadedPois,
    Dijkstra,
}

impl UiState {
    pub fn new(
        oracle: Arc<RwLock<Option<Oracle<Poi>>>>,
        positions: Arc<RwLock<MapPositions>>,
        sender: Sender<Events>,
    ) -> Self {
        Self {
            positions,
            oracle,
            sender,
            state: State::Init,
        }
    }
}

pub fn run_ui(state: &mut UiState, ctx: &Context) {
    egui::Window::new("Galileo map").show(ctx, |ui| {
        ui.label("Pointer position:");
        if let Some(pointer_position) = state.positions.read().expect("poisoned lock").pointer_pos()
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
        if let Some(map_center_position) = state
            .positions
            .read()
            .expect("poisoned lock")
            .map_center_pos()
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

    egui::SidePanel::right("Left panel").show(ctx, |ui| {
        if ui.add(egui::Button::new("Load graph")).clicked() {
            let file_path = FileDialog::new().pick_file().unwrap();

            state.sender.send(Events::LoadGraphFromPath(file_path));

            state.state = State::LoadedGraph;
        }

        if ui
            .add_enabled(
                state.state == State::LoadedGraph,
                egui::Button::new("Load POIs"),
            )
            .clicked()
        {
            let sender_clone = state.sender.clone();
            let file_path = FileDialog::new().set_directory("~/").pick_file().unwrap();
            let oracle_ref = Arc::clone(&state.oracle);

            tokio::spawn(async move {
                let file = File::open(file_path).unwrap();
                let buf_reader = BufReader::new(file);

                let mut poi_writer = PoiWriter::new(|_| true);
                read_geojson(buf_reader, &mut poi_writer);

                if let Some(ref mut oracel) = *oracle_ref.write().expect("poisoned lock") {
                    oracel.add_pois(poi_writer.pois());
                }
                info!("Loaded Pois");
            });

            state.state = State::LoadedPois;
        }
    });
}
