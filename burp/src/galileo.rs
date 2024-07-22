use std::sync::{Arc, RwLock};

use galileo::{
    error::GalileoError,
    galileo_types::{
        geo::{impls::GeoPoint2d, Crs, GeoPoint, NewGeoPoint},
        geometry_type::GeoSpace2d,
        Disambig, Disambiguate,
    },
    layer::{
        feature_layer::{self, Feature},
        FeatureLayer,
    },
    symbol::ArbitraryGeometrySymbol,
    Map,
};
use geo::LineString;
use geo_types;
use graph_rs::{CoordGraph, Coordinate};

pub struct GalileoMap {
    map: Arc<RwLock<Map>>,
    point_layer:
        Arc<RwLock<FeatureLayer<GeoPoint2d, NodeMarker, ArbitraryGeometrySymbol, GeoSpace2d>>>,
    graph_layer: Arc<
        RwLock<
            FeatureLayer<
                geo_types::Coord,
                Disambig<geo_types::LineString, GeoSpace2d>,
                ArbitraryGeometrySymbol,
                GeoSpace2d,
            >,
        >,
    >,
}

pub struct NodeMarker {
    point: GeoPoint2d,
    node: usize,
}

impl NodeMarker {
    pub fn new(point: GeoPoint2d, node: usize) -> Self {
        Self { point, node }
    }
    pub fn node(&self) -> usize {
        self.node
    }
}

impl Feature for NodeMarker {
    type Geom = GeoPoint2d;
    fn geometry(&self) -> &Self::Geom {
        &self.point
    }
}

impl GalileoMap {
    pub fn new(map: Arc<RwLock<Map>>) -> Self {
        let point_layer: FeatureLayer<_, NodeMarker, ArbitraryGeometrySymbol, GeoSpace2d> =
            FeatureLayer::new(vec![], ArbitraryGeometrySymbol::default(), Crs::WGS84);

        let graph_layer: FeatureLayer<
            _,
            Disambig<geo_types::LineString, GeoSpace2d>,
            ArbitraryGeometrySymbol,
            GeoSpace2d,
        > = FeatureLayer::new(vec![], ArbitraryGeometrySymbol::default(), Crs::WGS84);

        let point_layer = Arc::new(RwLock::new(point_layer));

        let graph_layer = Arc::new(RwLock::new(graph_layer));

        {
            let mut map = map.write().expect("poisoned lock");

            let mut layers = map.layers_mut();
            layers.push(graph_layer.clone());
            layers.push(point_layer.clone());
        }

        Self {
            map,
            point_layer,
            graph_layer,
        }
    }

    pub fn draw_node(&self, node: NodeMarker) -> Result<(), GalileoError> {
        let mut features = self.point_layer.write().expect("poisoned lock");

        features.features_mut().insert(node);

        Ok(())
    }

    pub fn draw_line(&self, edge: geo_types::LineString) -> Result<(), GalileoError> {
        let mut graph_layer = self.graph_layer.write().expect("poisoned lock");

        graph_layer.features_mut().insert(edge.to_geo2d());

        Ok(())
    }

    pub fn draw_coord_graph<T, EV, NV>(&self, graph: &T)
    where
        T: CoordGraph<EV, NV>,
        NV: Coordinate<f64>,
    {
        let nodes = graph.iter();

        for node in nodes {
            let p_1 = node.1.as_coord();
            for target in graph.neighbors(node.0) {
                let mut graph_layer = self.graph_layer.write().expect("poisoned lock");

                if let Some(node_value) = graph.node_value(target.target()) {
                    let p_2 = node_value.as_coord();
                    let line_string = LineString::new(vec![p_1, p_2]);

                    graph_layer.features_mut().insert(line_string.to_geo2d());
                }
            }
        }
    }
}
