use galileo::{
    control::{EventPropagation, UserEvent, UserEventHandler},
    layer::FeatureLayer,
    layer::Layer as GalileoLayer,
    symbol::Symbol,
    Map,
};
use galileo_types::{
    cartesian::NewCartesianPoint2d,
    geo::{
        impls::projection::{self, WebMercator},
        Crs, Datum, NewGeoPoint,
    },
    geometry::{Geom, Geometry},
    geometry_type::CartesianSpace2d,
    impls::Contour,
};
use geo::LineString;
use geo_types::{geometry::Coord, CoordNum};
use graph_rs::{CoordGraph, Coordinate};
use maybe_sync::{MaybeSend, MaybeSync};
use nalgebra::Scalar;
use num_traits::{Bounded, Float, FromPrimitive, Num};

use super::EventLayer;

pub trait RenderToContourLayer<C>
where
    C: CoordNum + Bounded + Scalar + FromPrimitive,
{
    fn get_features(&self) -> Vec<Contour<Coord<C>>>;
}

pub struct ContourLayer<S, C>
where
    S: Symbol<Contour<Coord<C>>>,
    C: CoordNum + Bounded + Scalar + FromPrimitive,
{
    layer: FeatureLayer<Coord<C>, Contour<Coord<C>>, S, CartesianSpace2d>,
}

impl<S, C> ContourLayer<S, C>
where
    S: Symbol<Contour<Coord<C>>>,
    C: CoordNum + Bounded + Scalar + FromPrimitive + Float,
    Coord<C>: NewCartesianPoint2d + NewGeoPoint,
{
    pub fn new(style: S) -> Self {
        Self {
            layer: FeatureLayer::with_lods(vec![], style, Crs::EPSG3857, &[8000.0, 1000.0, 1.0]),
        }
    }

    pub fn insert_line(&mut self, line: LineString<C>) {
        let projection: WebMercator<Coord<C>, Coord<C>> = WebMercator::new(Datum::WGS84);
        let line = line.project(&projection).unwrap();
        let Geom::Contour(contour) = line else {
            return;
        };
        self.layer.features_mut().insert(contour);
    }

    pub fn insert_lines(&mut self, lines: Vec<LineString<C>>) {
        lines.into_iter().for_each(|line| self.insert_line(line));
    }

    pub fn insert_coord_graph<T>(&mut self, graph: &T)
    where
        T: CoordGraph,
        T::NV: Coordinate<C> + Send + Sync,
        T::EV: Send + Sync,
    {
        let nodes = graph.nodes_iter();

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

    pub fn insert_features_from(&mut self, from: impl RenderToContourLayer<C>) {
        let features = self.layer.features_mut();

        from.get_features()
            .into_iter()
            .for_each(|f| features.insert(f));
    }
}

impl<S, C> GalileoLayer for ContourLayer<S, C>
where
    S: Symbol<Contour<Coord<C>>> + MaybeSend + MaybeSync + 'static,
    C: CoordNum + Bounded + Scalar + FromPrimitive + MaybeSend + MaybeSync,
    Coord<C>: NewCartesianPoint2d + NewGeoPoint,
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

impl<S, C> EventLayer for ContourLayer<S, C>
where
    S: Symbol<Contour<Coord<C>>> + MaybeSend + MaybeSync + 'static,
    C: CoordNum + Bounded + Scalar + FromPrimitive + MaybeSend + MaybeSync,
    Coord<C>: NewCartesianPoint2d + NewGeoPoint,
{
    fn handle_event(&self, event: &UserEvent, map: &mut Map) {}
}

impl<S, C> UserEventHandler for ContourLayer<S, C>
where
    S: Symbol<Contour<Coord<C>>> + MaybeSend + MaybeSync + 'static,
    C: CoordNum + Bounded + Scalar + FromPrimitive + MaybeSend + MaybeSync,
    Coord<C>: NewCartesianPoint2d + NewGeoPoint,
{
    fn handle(&self, event: &UserEvent, map: &mut Map) -> EventPropagation {
        self.handle_event(event, map);
        EventPropagation::Propagate
    }
}
