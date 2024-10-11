use galileo::{
    control::{EventPropagation, UserEvent, UserEventHandler},
    layer::FeatureLayer,
    layer::Layer as GalileoLayer,
    symbol::Symbol,
    Map,
};
use galileo_types::{
    geo::{
        impls::projection::{self, WebMercator},
        Crs, Datum,
    },
    geometry::{Geom, Geometry},
    geometry_type::CartesianSpace2d,
    impls::Contour,
};
use geo::LineString;
use geo_types::geometry::Coord;
use graph_rs::{CoordGraph, Coordinate};
use maybe_sync::{MaybeSend, MaybeSync};

use super::EventLayer;

pub struct ContourLayer<S>
where
    S: Symbol<Contour<Coord>>,
{
    layer: FeatureLayer<Coord, Contour<Coord>, S, CartesianSpace2d>,
}

impl<S> ContourLayer<S>
where
    S: Symbol<Contour<Coord>>,
{
    pub fn new(style: S) -> Self {
        Self {
            layer: FeatureLayer::with_lods(vec![], style, Crs::EPSG3857, &[8000.0, 1000.0, 1.0]),
        }
    }

    pub fn insert_line(&mut self, line: LineString) {
        let projection: WebMercator<Coord, Coord> = WebMercator::new(Datum::WGS84);
        let line = line.project(&projection).unwrap();
        let Geom::Contour(contour) = line else {
            return;
        };
        self.layer.features_mut().insert(contour);
    }

    pub fn insert_lines(&mut self, lines: Vec<LineString>) {
        lines.into_iter().for_each(|line| self.insert_line(line));
    }

    pub fn insert_coord_graph<T, EV, NV>(&mut self, graph: &T)
    where
        T: CoordGraph<EV, NV>,
        NV: Coordinate<f64> + Send + Sync,
        EV: Send + Sync,
    {
        let nodes = graph.iter();

        for node in nodes {
            let p_1 = node.1.as_coord();
            for target in graph.neighbors(node.0) {
                if let Some(node_value) = graph.node_value(target.target()) {
                    let p_2 = node_value.as_coord();
                    let line = LineString::new(vec![p_1, p_2]);

                    self.insert_line(line);
                }
            }
        }
    }
}

impl<S> GalileoLayer for ContourLayer<S>
where
    S: Symbol<Contour<Coord>> + MaybeSend + MaybeSync + 'static,
{
    fn render(&self, view: &galileo::MapView, canvas: &mut dyn galileo::render::Canvas) {
        self.layer.render(view, canvas)
    }

    fn prepare(&self, view: &galileo::MapView) {
        self.layer.prepare(view)
    }

    fn set_messenger(&mut self, messenger: Box<dyn galileo::Messenger>) {
        self.layer.set_messenger(messenger)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl<S> EventLayer for ContourLayer<S>
where
    S: Symbol<Contour<Coord>> + MaybeSend + MaybeSync + 'static,
{
    fn handle_event(&self, event: &UserEvent, map: &mut Map) {}
}

impl<S> UserEventHandler for ContourLayer<S>
where
    S: Symbol<Contour<Coord>> + MaybeSend + MaybeSync + 'static,
{
    fn handle(&self, event: &UserEvent, map: &mut Map) -> EventPropagation {
        self.handle_event(event, map);
        EventPropagation::Propagate
    }
}
