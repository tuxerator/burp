extern crate geozero;

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};

use burp::oracle::{self, Oracle};
use burp::types::{CoordNode, Poi};
use egui::{Context, Id};
use egui_graphs::Node;
use galileo_types::geo::impls::GeoPoint2d;
use galileo_types::geo::{GeoPoint, NewGeoPoint};
use geo::{Area, Coord};
use geozero::geojson::read_geojson;
use graph_rs::algorithms::dijkstra::Dijkstra;
use graph_rs::graph::csr::DirectedCsrGraph;
use graph_rs::graph::quad_tree::QuadGraph;
use graph_rs::{CoordGraph, DirectedGraph, Graph};
use log::info;
use ordered_float::OrderedFloat;
use rfd::FileDialog;

use crate::map::Map;
use crate::state::Events;
use crate::types::MapPositions;
use burp::input::geo_zero::{ColumnValueClonable, GraphWriter, PoiWriter};

pub struct UiState {
    oracle: Arc<RwLock<Option<Oracle<Poi>>>>,
    map: Arc<RwLock<Map<String>>>,
    sender: Sender<Events>,
    state: State,
}

#[derive(PartialEq)]
enum State {
    Init,
    LoadedGraph,
    LoadedPois,
    Dijkstra(Option<(usize, CoordNode<Poi>)>),
}

impl UiState {
    pub fn new(
        oracle: Arc<RwLock<Option<Oracle<Poi>>>>,
        map: Arc<RwLock<Map<String>>>,
        sender: Sender<Events>,
    ) -> Self {
        Self {
            oracle,
            map,
            sender,
            state: State::Init,
        }
    }
}

pub fn run_ui(state: &mut UiState, ctx: &Context) {
    match state.state {
        State::Dijkstra(_) => dijkstra(state),
        _ => (),
    };

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

                    info!("Loaded Pois");
                }
            });

            state.state = State::LoadedPois;
        }

        if ui
            .add_enabled(
                state.state == State::LoadedPois,
                egui::Button::new("Dijkstra"),
            )
            .clicked()
        {
            state
                .positions
                .write()
                .expect("poisoned lock")
                .take_click_pos();
            state.state = State::Dijkstra(None);
        }

        if ui.button("Save").clicked() {
            if let Some(path) = FileDialog::new().save_file() {
                if let Ok(file) = File::create(path) {
                    if let Some(ref oracle) = *state.oracle.read().expect("poisoned lock") {
                        let mut buf_writer = BufWriter::new(file);
                        let _ = buf_writer.write(oracle.to_flexbuffer().as_slice());
                    };
                };
            };
        }

        if ui.button("Load").clicked() {
            if let Some(path) = FileDialog::new().pick_file() {
                if let Ok(file) = File::open(path) {
                    let mut oracle = state.oracle.write().expect("poisoned lock");
                    let mut buf_reader = BufReader::new(file);

                    let mut flexbuffer = Vec::default();

                    buf_reader.read_to_end(&mut flexbuffer);
                    *oracle = Some(Oracle::read_flexbuffer(flexbuffer.as_slice()));
                    state.state = State::LoadedPois;
                }
            }
        }
    });
}

fn dijkstra(state: &mut UiState) {
    if let Some(ref oracle) = *state.oracle.read().expect("poisoned lock") {
        let State::Dijkstra(Some(ref start)) = state.state else {
            info!("Waiting for start position");
            let click_pos = state
                .positions
                .write()
                .expect("poisoned lock")
                .take_click_pos();
            state.state = State::Dijkstra(click_pos.and_then(|click_pos| {
                oracle
                    .get_node_value_at(geo::Coord::lonlat(click_pos.lon(), click_pos.lat()), 100.0)
                    .ok()
            }));
            return;
        };

        let Some(end) = state
            .positions
            .write()
            .expect("poisoned lock")
            .take_click_pos()
            .and_then(|click_pos| {
                oracle
                    .get_node_value_at(geo::Coord::lonlat(click_pos.lon(), click_pos.lat()), 100.0)
                    .ok()
            })
        else {
            return;
        };
        info!(
            "Calculating shortes path from node {:?} to node {:?}",
            &start, &end
        );
        let mut target = HashSet::new();
        target.insert(end.0);
        let result = oracle.dijkstra(start.0, target);
    } else {
        info!("Couldn't get a lock on oracle");
    }
}
