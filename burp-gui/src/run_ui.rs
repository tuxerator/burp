extern crate geozero;

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::f64;
use std::fmt::Debug;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};

use burp::graph::{oracle, PoiGraph};
use burp::types::{CoordNode, Poi};
use egui::{Context, InnerResponse};
use galileo::symbol::{CirclePointSymbol, SimpleContourSymbol};
use galileo::Color;
use galileo_types::geo::{GeoPoint, NewGeoPoint};
use geo::{coord, Coord, LineString};
use geo_types::geometry::Polygon;
use geozero::geojson::read_geojson;
use graph_rs::graph::quad_tree::QuadGraph;
use graph_rs::graph::rstar::RTreeGraph;
use graph_rs::Graph;
use log::{info, warn};
use rfd::FileDialog;

use crate::map::layers::poly_layer::{BlocksLayer, BlocksSymbol};
use crate::map::layers::{
    line_layer::ContourLayer,
    node_layer::{NodeLayer, NodeMarker},
};
use crate::map::Map;
use crate::state::Events;
use burp::input::geo_zero::{ColumnValueClonable, GraphWriter, PoiWriter};

type StartEndPos<T> = (
    Option<(usize, CoordNode<f64, T>)>,
    Option<(usize, CoordNode<f64, T>)>,
);

pub struct UiState {
    oracle: Option<PoiGraph<Poi>>,
    map: Arc<RwLock<Map<String>>>,
    sender: Sender<Events>,
    state: State,
    epsilon: f64,
}

#[derive(PartialEq)]
enum State {
    Init,
    LoadedGraph,
    LoadedPois,
    Dijkstra(
        (
            Option<(usize, CoordNode<f64, Poi>)>,
            Option<(usize, CoordNode<f64, Poi>)>,
        ),
    ),
    DoubleDijkstra(
        (
            Option<(usize, CoordNode<f64, Poi>)>,
            Option<(usize, CoordNode<f64, Poi>)>,
        ),
    ),
    DoubleDijkstraResult(HashMap<usize, f64>),
    Oracle(Option<(usize, CoordNode<f64, Poi>)>),
}

impl Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            State::Init => write!(f, "Init"),
            State::LoadedGraph => write!(f, "LoadedGraph"),
            State::LoadedPois => write!(f, "LoadedPois"),
            State::Dijkstra(points) => write!(f, "Dijkstra({:?})", points),
            State::DoubleDijkstra(points) => write!(f, "DoubleDijkstra({:?})", points),
            State::DoubleDijkstraResult(_) => write!(f, "DoubleDijkstraResult"),
            State::Oracle(point) => write!(f, "Oracle({:?})", point),
        }
    }
}

impl UiState {
    pub fn new(map: Arc<RwLock<Map<String>>>, sender: Sender<Events>) -> Self {
        Self {
            oracle: None,
            map,
            sender,
            state: State::Init,
            epsilon: 0.2,
        }
    }
}

pub fn run_ui(state: &mut UiState, ctx: &Context) {
    match state.state {
        State::Dijkstra(_) => dijkstra(state),
        State::DoubleDijkstra(_) => double_dijkstra(state),
        State::DoubleDijkstraResult(_) => draw_resutl_path(state),
        State::Oracle(_) => build_oracle(state),
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

        ui.separator();

        ui.label("State:");
        ui.label(format!("{:?}", state.state));
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
                    Some(ColumnValueClonable::String(s)) if s == "cycleway" => return false,
                    Some(ColumnValueClonable::String(s)) if s == "path" => return false,
                    Some(ColumnValueClonable::String(s)) if s == "footway" => return false,
                    Some(ColumnValueClonable::String(s)) if s == "steps" => return false,
                    Some(ColumnValueClonable::String(s)) if s == "corridor" => return false,
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
            let graph = RTreeGraph::new_from_graph(graph_writer.get_graph());
            state.oracle = Some(PoiGraph::new(graph));

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
            let file_path = FileDialog::new().pick_file().unwrap();

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
            .add_enabled(state.oracle.is_some(), egui::Button::new("Toggle graph"))
            .clicked()
        {
            let mut map = state.map.write().expect("poisoned lock");

            map.toggle_layer(&String::from("graph"))
                .or_else(|_| -> Result<(), String> {
                    let layer: &mut Arc<RwLock<ContourLayer<SimpleContourSymbol, f64>>> = map
                        .or_insert(
                            "graph".to_string(),
                            ContourLayer::new(SimpleContourSymbol::new(Color::GREEN, 2.0)),
                        )
                        .as_any_mut()
                        .downcast_mut()
                        .ok_or("Couldn't downcast layer".to_string())?;

                    layer
                        .write()
                        .expect("poisoned lock")
                        .insert_coord_graph(&*state.oracle.as_ref().unwrap().graph());

                    Ok(())
                });
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
            state.state = State::Dijkstra((None, None));
        }

        if ui
            .add_enabled(
                state.state == State::LoadedPois,
                egui::Button::new("Double Dijkstra"),
            )
            .clicked()
        {
            state
                .map
                .write()
                .expect("poisoned lock")
                .take_click_pos()
                .expect("poisoned lock");
            state.state = State::DoubleDijkstra((None, None));
        }

        if ui
            .add_enabled(
                matches!(state.state, State::LoadedPois | State::LoadedGraph),
                egui::Button::new("Build Oracle"),
            )
            .clicked()
        {
            state
                .map
                .write()
                .expect("poisoned lock")
                .take_click_pos()
                .expect("poisoned lock");
            state.state = State::Oracle(None);
        }

        ui.add(egui::Slider::new(&mut state.epsilon, 0.0..=1.0));

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
                    state.oracle = Some(PoiGraph::read_flexbuffer(flexbuffer.as_slice()));
                    state.state = State::LoadedPois;
                }
            }
        }
    });
}

fn dijkstra(state: &mut UiState) {
    if let Some(ref oracle) = state.oracle {
        let State::Dijkstra((Some(ref start), Some(ref end))) = state.state else {
            let State::Dijkstra(ref mut pos) = state.state else {
                return;
            };
            *pos = get_start_end_pos(pos.clone(), state.map.clone(), oracle);
            return;
        };
        info!(
            "Calculating shortes path from node {:?} to node {:?}",
            &start, &end
        );
        let mut target = HashSet::new();
        target.insert(end.0);
        let result = oracle
            .dijkstra(start.0, target, graph_rs::types::Direction::Outgoing)
            .unwrap();

        let mut layer = state.map.write().expect("poisoned lock");

        let path = result.path(end.0).unwrap();

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

        let layer: &mut Arc<RwLock<ContourLayer<SimpleContourSymbol, f64>>> = layer
            .or_insert(
                "path".to_string(),
                ContourLayer::new(SimpleContourSymbol::new(Color::BLUE, 1.0)),
            )
            .as_any_mut()
            .downcast_mut()
            .unwrap();
        layer
            .write()
            .expect("poisoned lock")
            .insert_line(LineString::new(coords));
    } else {
        warn!("Couldn't get a lock on oracle");
    }

    state.state = State::LoadedPois;
}

fn double_dijkstra(state: &mut UiState) {
    if let Some(ref oracle) = state.oracle {
        let State::DoubleDijkstra((Some(ref start), Some(ref end))) = state.state else {
            let State::DoubleDijkstra(ref mut pos) = state.state else {
                return;
            };
            *pos = get_start_end_pos(pos.clone(), state.map.clone(), oracle);
            return;
        };
        let mut target = HashSet::new();
        target.insert(end.0);
        let result = oracle.beer_path_dijkstra_fast(start.0, end.0, oracle.poi_nodes(), 0.0);

        info!("Finished beer paths");

        state.state = State::DoubleDijkstraResult(result);
    } else {
        warn!("Couldn't get a lock on oracle");
        state.state = State::LoadedPois;
    }
}

fn draw_resutl_path(state: &mut UiState) {
    let State::DoubleDijkstraResult(ref result) = state.state else {
        return;
    };
    let Some(ref oracle) = state.oracle else {
        return;
    };

    let pois = result
        .iter()
        .fold(vec![], |mut coords: Vec<NodeMarker<Poi>>, node| {
            if let Some(node_marker) = NodeMarker::new(
                *oracle.graph().node_value(*node.0).unwrap().get_coord(),
                *node.0,
                Some(oracle.graph().node_value(*node.0).unwrap().data().to_vec()),
            ) {
                coords.push(node_marker);
            }
            coords
        });
    let mut map = state.map.write().expect("poisoned lock");
    {
        let layer: &mut Arc<RwLock<NodeLayer<CirclePointSymbol, Poi>>> = map
            .or_insert(
                "points".to_string(),
                NodeLayer::<CirclePointSymbol, Poi>::new(CirclePointSymbol::new(Color::RED, 5.0)),
            )
            .as_any_mut()
            .downcast_mut()
            .unwrap();

        let mut layer = layer.write().expect("poisoned lock");
        layer.insert_nodes(pois);
    }
    map.show_layer(&"points".to_string());

    state.state = State::LoadedPois;
}

fn get_start_end_pos(
    pos: StartEndPos<Poi>,
    map: Arc<RwLock<Map<String>>>,
    oracle: &PoiGraph<Poi>,
) -> StartEndPos<Poi> {
    match pos {
        (None, None) => (get_node_click_pos(map, oracle), None),
        (Some(start), None) => (Some(start), get_node_click_pos(map, oracle)),
        pos => pos,
    }
}

fn get_node_click_pos(
    map: Arc<RwLock<Map<String>>>,
    oracle: &PoiGraph<Poi>,
) -> Option<(usize, CoordNode<f64, Poi>)> {
    let click_pos = map
        .write()
        .expect("poisoned lock")
        .take_click_pos()
        .expect("poisoned lock");
    click_pos.and_then(|click_pos| {
        oracle
            .get_node_value_at(&geo::Coord::lonlat(click_pos.lon(), click_pos.lat()), 100.0)
            .ok()
    })
}

fn build_oracle(state: &mut UiState) {
    let Some(ref graph) = state.oracle else {
        warn!("No graph loaded");
        return;
    };

    let State::Oracle(Some(ref pos)) = state.state else {
        let node = get_node_click_pos(state.map.clone(), graph);
        state.state = State::Oracle(node);
        return;
    };

    let oracle = oracle::build(&mut graph.graph_mut(), pos.0, state.epsilon);

    let mut map = state.map.write().expect("poisoned lock");

    state.state = State::LoadedPois;
}

fn unwrap_error(
    response: Option<InnerResponse<Option<Result<(), Box<dyn Error>>>>>,
) -> Result<(), Box<dyn Error>> {
    response.map_or(Ok(()), |inner_response| {
        inner_response.inner.map_or(Ok(()), |res| res)
    })
}
