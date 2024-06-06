use egui::Context;
use egui_graphs::{DefaultEdgeShape, DefaultNodeShape, Graph, GraphView};
use galileo_types::geo::impls::GeoPoint2d;
use galileo_types::geo::GeoPoint;
use petgraph::stable_graph::StableGraph;
use petgraph::Directed;

#[derive(Clone, Debug)]
pub struct UiState {
    pub positions: Positions,
    pub map_hidden: bool,
    pub graph: Graph<(), (), Directed>,
}

impl UiState {
    pub fn new() -> Self {
        let mut g: StableGraph<(), ()> = StableGraph::new();

        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());

        g.add_edge(a, b, ());
        g.add_edge(b, c, ());
        g.add_edge(c, a, ());

        Self {
            positions: Positions::default(),
            map_hidden: false,
            graph: Graph::from(&g),
        }
    }
}

#[derive(Clone, Default, Debug)]
pub struct Positions {
    pub pointer_position: Option<GeoPoint2d>,
    pub map_center_position: Option<GeoPoint2d>,
}

pub fn run_ui(state: &mut UiState, ui: &Context) {
    egui::Window::new("Galileo map").show(ui, |ui| {
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

    egui::SidePanel::right("Left panel").show(ui, |ui| {
        if ui.add(egui::Button::new("Hide map")).clicked() {
            state.map_hidden = true;
        }
    });
}
