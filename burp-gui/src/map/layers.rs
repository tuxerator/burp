use galileo::{
    error::GalileoError,
    layer::{feature_layer::Feature, FeatureLayer, Layer as GalileoLayer},
    symbol::Symbol,
};
use galileo_types::{
    geo::{impls::GeoPoint2d, Crs},
    geometry_type::GeoSpace2d,
    Disambig, Disambiguate,
};
use geo::{Coord, LineString};
use graph_rs::{CoordGraph, Coordinate};
use maybe_sync::{MaybeSend, MaybeSync};

pub struct NodeMarker<T> {
    point: GeoPoint2d,
    node: usize,
    data: Option<Vec<T>>,
}

impl<T> NodeMarker<T> {
    pub fn new(point: GeoPoint2d, node: usize, data: Option<Vec<T>>) -> Self {
        Self { point, node, data }
    }

    pub fn node(&self) -> usize {
        self.node
    }

    pub fn data(&self) -> Option<&Vec<T>> {
        self.data.as_ref()
    }
}

impl<T> Feature for NodeMarker<T> {
    type Geom = GeoPoint2d;
    fn geometry(&self) -> &Self::Geom {
        &self.point
    }
}

pub struct NodeLayer<S, T>
where
    S: Symbol<NodeMarker<T>>,
{
    layer: FeatureLayer<GeoPoint2d, NodeMarker<T>, S, GeoSpace2d>,
}

impl<S, T> NodeLayer<S, T>
where
    S: Symbol<NodeMarker<T>>,
{
    pub fn new(style: S) -> Self {
        Self {
            layer: FeatureLayer::new(vec![], style, Crs::WGS84),
        }
    }

    pub fn insert_node(&mut self, node: NodeMarker<T>) {
        self.layer.features_mut().insert(node);
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
        self.layer.as_any()
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self.layer.as_any_mut()
    }
}

pub struct LineLayer<S>
where
    S: Symbol<Disambig<LineString, GeoSpace2d>>,
{
    layer: FeatureLayer<Coord, Disambig<LineString, GeoSpace2d>, S, GeoSpace2d>,
}

impl<S> LineLayer<S>
where
    S: Symbol<Disambig<LineString, GeoSpace2d>>,
{
    pub fn new(style: S) -> Self {
        Self {
            layer: FeatureLayer::new(vec![], style, Crs::WGS84),
        }
    }

    pub fn insert_line(&mut self, line: LineString) {
        self.layer.features_mut().insert(line.to_geo2d());
    }

    pub fn insert_lines(&mut self, lines: Vec<LineString>) {
        lines
            .into_iter()
            .for_each(|line| self.layer.features_mut().insert(line.to_geo2d()));
    }

    pub fn insert_coord_graph<T, EV, NV>(&mut self, graph: &T)
    where
        T: CoordGraph<EV, NV>,
        NV: Coordinate<f64>,
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
    S: Symbol<Disambig<LineString, GeoSpace2d>> + MaybeSend + MaybeSync + 'static,
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
        self.layer.as_any()
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self.layer.as_any_mut()
    }
}
