use galileo::{
    Map,
    control::{EventPropagation, UserEvent, UserEventHandler},
    layer::{FeatureId, FeatureLayer, Layer as GalileoLayer},
    symbol::Symbol,
};
use galileo_types::{
    Disambig, Disambiguate,
    cartesian::NewCartesianPoint2d,
    geo::{Crs, Datum, NewGeoPoint, impls::projection::WebMercator},
    geometry::{Geom, Geometry},
    geometry_type::{CartesianSpace2d, GeoSpace2d},
    impls::Contour,
};
use geo::LineString;
use geo_types::{CoordNum, geometry::Coord};
use graph_rs::{CoordGraph, Coordinate};
use maybe_sync::{MaybeSend, MaybeSync};
use nalgebra::Scalar;
use num_traits::{Bounded, Float, FromPrimitive};

use super::EventLayer;

pub trait RenderToContourLayer<C>
where
    C: CoordNum + Bounded + Scalar + FromPrimitive,
{
    fn get_features(&self) -> Vec<Contour<Coord<C>>>;
}

pub struct ContourLayer<S, C>
where
    S: Symbol<Disambig<LineString<C>, GeoSpace2d>>,
    C: CoordNum + Bounded + Scalar + FromPrimitive,
{
    layer: FeatureLayer<
        <Disambig<LineString<C>, GeoSpace2d> as Geometry>::Point,
        Disambig<LineString<C>, GeoSpace2d>,
        S,
        GeoSpace2d,
    >,
}

impl<S, C> ContourLayer<S, C>
where
    S: Symbol<Disambig<LineString<C>, GeoSpace2d>>,
    C: CoordNum + Bounded + Scalar + FromPrimitive + Float + MaybeSend + MaybeSync,
    Coord<C>: NewCartesianPoint2d + NewGeoPoint,
{
    pub fn new(style: S, crs: Crs) -> Self {
        Self {
            layer: FeatureLayer::with_lods(vec![], style, crs, &[8000.0, 1000.0, 1.0]),
        }
    }

    pub fn insert_line(&mut self, line: LineString<C>) -> FeatureId {
        self.layer.features_mut().add(line.to_geo2d())
    }

    pub fn insert_lines(&mut self, lines: Vec<LineString<C>>) -> Vec<FeatureId> {
        lines
            .into_iter()
            .map(|line| self.insert_line(line))
            .collect()
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

    // pub fn insert_features_from(&mut self, from: impl RenderToContourLayer<C>) {
    //     let features = self.layer.features_mut();
    //
    //     from.get_features()
    //         .into_iter()
    //         .for_each(|f| features.add(f));
    // }
}

impl<S, C> GalileoLayer for ContourLayer<S, C>
where
    S: Symbol<Disambig<LineString<C>, GeoSpace2d>> + MaybeSend + MaybeSync + 'static,
    C: CoordNum + Bounded + Scalar + FromPrimitive + MaybeSend + MaybeSync,
    Coord<C>: NewGeoPoint,
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

    fn attribution(&self) -> Option<galileo::layer::attribution::Attribution> {
        None
    }
}

impl<S, C> EventLayer for ContourLayer<S, C>
where
    S: Symbol<Disambig<LineString<C>, GeoSpace2d>> + MaybeSend + MaybeSync + 'static,
    C: CoordNum + Bounded + Scalar + FromPrimitive + MaybeSend + MaybeSync,
    Coord<C>: NewGeoPoint,
{
    fn handle_event(&self, event: &UserEvent, map: &mut Map) {}
}

impl<S, C> UserEventHandler for ContourLayer<S, C>
where
    S: Symbol<Disambig<LineString<C>, GeoSpace2d>> + MaybeSend + MaybeSync + 'static,
    C: CoordNum + Bounded + Scalar + FromPrimitive + MaybeSend + MaybeSync,
    Coord<C>: NewCartesianPoint2d + NewGeoPoint,
{
    fn handle(&self, event: &UserEvent, map: &mut Map) -> EventPropagation {
        self.handle_event(event, map);
        EventPropagation::Propagate
    }
}
