extern crate geozero;

use std::any::TypeId;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};

use burp::oracle::{self, Oracle};
use burp::types::{CoordNode, Poi};
use egui::{Context, Id, InnerResponse};
use egui_graphs::Node;
use galileo::symbol::{ArbitraryGeometrySymbol, SimpleContourSymbol};
use galileo::Color;
use galileo_types::geo::impls::GeoPoint2d;
use galileo_types::geo::{GeoPoint, NewGeoPoint};
use geo::{Area, Coord, LineString};
use geozero::geojson::read_geojson;
use graph_rs::algorithms::dijkstra::Dijkstra;
use graph_rs::graph::csr::DirectedCsrGraph;
use graph_rs::graph::quad_tree::QuadGraph;
use graph_rs::{CoordGraph, DirectedGraph, Graph};
use log::info;
use ordered_float::OrderedFloat;
use rfd::FileDialog;

use crate::map::layers::LineLayer;
use crate::map::Map;
use crate::state::Events;
use crate::types::MapPositions;
use burp::input::geo_zero::{ColumnValueClonable, GraphWriter, PoiWriter};

pub struct UiState {
    oracle: Option<Oracle<Poi>>,
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
    pub fn new(map: Arc<RwLock<Map<String>>>, sender: Sender<Events>) -> Self {
        Self {
            oracle: None,
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
        if let Some(pointer_position) = state
            .map
            .read()
            .expect("poisoned lock")
            .pointer_pos()
            .expect("poisoned lock")
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
            .map
            .read()
            .expect("poisoned lock")
            .map_center_pos()
            .expect("poisoned lock")
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
            state.oracle = Some(Oracle::new(graph));

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

            let file = File::open(file_path).unwrap();
            let buf_reader = BufReader::new(file);

            let mut poi_writer = PoiWriter::new(|_| true);
            read_geojson(buf_reader, &mut poi_writer);

            if let Some(ref mut oracel) = state.oracle {
                oracel.add_pois(poi_writer.pois());

                info!("Loaded Pois");
            }

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
                .map
                .write()
                .expect("poisoned lock")
                .take_click_pos()
                .expect("poisoned lock");
            state.state = State::Dijkstra(None);
        }

        if ui.button("Save").clicked() {
            if let Some(path) = FileDialog::new().save_file() {
                if let Ok(file) = File::create(path) {
                    if let Some(ref oracle) = state.oracle {
                        let mut buf_writer = BufWriter::new(file);
                        let _ = buf_writer.write(oracle.to_flexbuffer().as_slice());
                    };
                };
            };
        }

        if ui.button("Load").clicked() {
            if let Some(path) = FileDialog::new().pick_file() {
                if let Ok(file) = File::open(path) {
                    let mut buf_reader = BufReader::new(file);

                    let mut flexbuffer = Vec::default();

                    buf_reader.read_to_end(&mut flexbuffer);
                    state.oracle = Some(Oracle::read_flexbuffer(flexbuffer.as_slice()));
                    state.state = State::LoadedPois;
                }
            }
        }
    });
}

fn dijkstra(state: &mut UiState) {
    if let Some(ref oracle) = state.oracle {
        let State::Dijkstra(Some(ref start)) = state.state else {
            info!("Waiting for start position");
            let click_pos = state
                .map
                .write()
                .expect("poisoned lock")
                .take_click_pos()
                .expect("poisoned lock");
            state.state = State::Dijkstra(click_pos.and_then(|click_pos| {
                oracle
                    .get_node_value_at(geo::Coord::lonlat(click_pos.lon(), click_pos.lat()), 100.0)
                    .ok()
            }));
            return;
        };

        let Some(end) = state
            .map
            .write()
            .expect("poisoned lock")
            .take_click_pos()
            .expect("poisoned lock")
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
        let result = oracle.dijkstra(start.0, target).unwrap();

        let mut layer = state.map.write().expect("poisoned lock");

        dbg!(&result);

        dbg!(result.get(end.0));

        let path = result.path(end.0).unwrap();

        dbg!(&path);

        let coords = path
            .into_iter()
            .fold(vec![], |mut coords: Vec<Coord>, node| {
                coords.push(
                    *state
                        .oracle
                        .as_ref()
                        .unwrap()
                        .graph()
                        .node_value(node.node_id())
                        .unwrap()
                        .get_coord(),
                );
                coords
            });

        info!("Path found {:?}", &coords);

        let layer: &mut Arc<RwLock<LineLayer<SimpleContourSymbol>>> = layer
            .or_insert(
                "path".to_string(),
                LineLayer::new(SimpleContourSymbol::new(Color::BLUE, 1.0)),
            )
            .as_any_mut()
            .downcast_mut()
            .unwrap();
        layer
            .write()
            .expect("poisoned lock")
            .insert_line(LineString::new(coords));
    } else {
        info!("Couldn't get a lock on oracle");
    }
}

fn unwrap_error(
    response: Option<InnerResponse<Option<Result<(), Box<dyn Error>>>>>,
) -> Result<(), Box<dyn Error>> {
    response.map_or(Ok(()), |inner_response| {
        inner_response.inner.map_or(Ok(()), |res| res)
    })
}
