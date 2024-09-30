use std::{
    fmt::Debug,
    sync::{Arc, RwLock},
};

use galileo::{
    control::{EventPropagation, MouseButton, MouseEvent, UserEvent, UserEventHandler},
    error::GalileoError,
    layer::{
        feature_layer::{Feature, FeatureStore},
        FeatureLayer, Layer as GalileoLayer,
    },
    symbol::Symbol,
    Map,
};
use galileo_types::{
    cartesian::CartesianPoint2d,
    geo::{
        impls::{
            projection::{self, WebMercator},
            GeoPoint2d,
        },
        Crs, Datum, GeoPoint, Projection,
    },
    geometry::{Geom, Geometry, GeometrySpecialization},
    geometry_type::{CartesianSpace2d, GeoSpace2d, GeometryType},
    impls::Contour,
    Disambig, Disambiguate,
};
use geo::{LineString, Point};
use geo_types::geometry::Coord;
use graph_rs::{CoordGraph, Coordinate};
use lazy_static::lazy_static;
use log::info;
use maybe_sync::{MaybeSend, MaybeSync};

pub struct NodeMarker<T> {
    coord: Disambig<Coord, CartesianSpace2d>,
    node: usize,
    data: Option<Vec<T>>,
}

impl<T> NodeMarker<T> {
    pub fn new(coord: Coord, node: usize, data: Option<Vec<T>>) -> Option<Self> {
        let projection = Crs::EPSG3857.get_projection()?;
        Some(Self {
            coord: projection.project(&coord)?,
            node,
            data,
        })
    }

    pub fn node(&self) -> usize {
        self.node
    }

    pub fn data(&self) -> Option<&Vec<T>> {
        self.data.as_ref()
    }
}

impl<T> Feature for NodeMarker<T> {
    type Geom = Disambig<Coord, CartesianSpace2d>;
    fn geometry(&self) -> &Self::Geom {
        &self.coord
    }
}

pub struct NodeLayer<S, T>
where
    S: Symbol<NodeMarker<T>>,
{
    layer: FeatureLayer<
        <Disambig<Coord, CartesianSpace2d> as Geometry>::Point,
        NodeMarker<T>,
        S,
        CartesianSpace2d,
    >,
}

impl<S, T> NodeLayer<S, T>
where
    S: Symbol<NodeMarker<T>>,
{
    pub fn new(style: S) -> Self {
        Self {
            layer: FeatureLayer::new(vec![], style, Crs::EPSG3857),
        }
    }

    pub fn insert_node(&mut self, node: NodeMarker<T>) {
        self.layer.features_mut().insert(node);
    }

    pub fn insert_nodes(&mut self, nodes: Vec<NodeMarker<T>>) {
        nodes.into_iter().for_each(|node| self.insert_node(node));
    }
}

impl<S, T> GalileoLayer for NodeLayer<S, T>
where
    S: Symbol<NodeMarker<T>> + MaybeSend + MaybeSync + 'static,
    T: MaybeSend + MaybeSync + 'static,
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

impl<S, T> EventLayer for NodeLayer<S, T>
where
    S: Symbol<NodeMarker<T>> + MaybeSend + MaybeSync + 'static,
    T: MaybeSend + MaybeSync + Debug + 'static,
{
    fn handle_event(&self, event: &UserEvent, map: &mut Map) {
        match event {
            UserEvent::Click(MouseButton::Left, event_data) => {
                let Some(position) = map.view().screen_to_map(event_data.screen_pointer_position)
                else {
                    return;
                };
                for feature in self
                    .layer
                    .get_features_at(&position, map.view().resolution() * 20.0)
                {
                    info!("Data: {:?}", feature.as_ref().data);
                }
            }
            _ => (),
        };
    }
}

pub struct LineLayer<S>
where
    S: Symbol<Contour<Coord>>,
{
    layer: FeatureLayer<Coord, Contour<Coord>, S, CartesianSpace2d>,
}

impl<S> LineLayer<S>
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

impl<S> GalileoLayer for LineLayer<S>
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

impl<S> EventLayer for LineLayer<S>
where
    S: Symbol<Contour<Coord>> + MaybeSend + MaybeSync + 'static,
{
    fn handle_event(&self, event: &UserEvent, map: &mut Map) {}
}

impl<S> UserEventHandler for LineLayer<S>
where
    S: Symbol<Contour<Coord>> + MaybeSend + MaybeSync + 'static,
{
    fn handle(&self, event: &UserEvent, map: &mut Map) -> EventPropagation {
        self.handle_event(event, map);
        EventPropagation::Propagate
    }
}

pub struct Layer<T>(Arc<RwLock<T>>);

pub trait EventLayer: GalileoLayer {
    fn handle_event(&self, event: &UserEvent, map: &mut Map);
}

impl<T> EventLayer for Arc<RwLock<T>>
where
    T: EventLayer + 'static,
{
    fn handle_event(&self, event: &UserEvent, map: &mut Map) {
        self.read().expect("poisoned lock").handle_event(event, map)
    }
}

impl<T> UserEventHandler for Layer<T>
where
    T: EventLayer,
{
    fn handle(&self, event: &UserEvent, map: &mut Map) -> EventPropagation {
        self.0
            .read()
            .expect("poisoned lock")
            .handle_event(event, map);
        EventPropagation::Propagate
    }
}
