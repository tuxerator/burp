use std::{fmt::Debug, sync::Arc};

use burp::oracle::block_pair::BlockPair;
use galileo::{
    Color, Messenger,
    layer::{FeatureId, FeatureLayer, Layer as GalileoLayer, feature_layer::Feature},
    symbol::{
        ArbitraryGeometrySymbol, CirclePointSymbol, SimpleContourSymbol, SimplePolygonSymbol,
    },
};
use galileo_types::{
    Disambig, Disambiguate, Geometry, MultiPolygon as MultiPolygonTrait,
    cartesian::{CartesianPoint2d, NewCartesianPoint2d},
    geo::{Crs, Datum, NewGeoPoint, impls::projection::WebMercator},
    geometry::Geom,
    geometry_type::{CartesianSpace2d, GeoSpace2d},
};
use geo::{Coord, CoordFloat, LineString, MultiPoint, MultiPolygon, Point, Polygon};
use graph_rs::{CoordGraph, algorithms::dijkstra::Dijkstra};
use log::info;
use nalgebra::Scalar;
use num_traits::{Bounded, FromPrimitive, Num, float::FloatCore};
use rstar::RTreeNum;
use rustc_hash::FxHashSet;

use crate::map::layers::EventLayer;

pub struct BlockPairLayer<C>
where
    C: RTreeNum + CoordFloat + FromPrimitive + 'static,
{
    poly_layer: FeatureLayer<
        <Disambig<MultiPolygon<C>, GeoSpace2d> as Geometry>::Point,
        Disambig<MultiPolygon<C>, GeoSpace2d>,
        SimplePolygonSymbol,
        GeoSpace2d,
    >,
    radius_layer: FeatureLayer<
        <Disambig<LineString<C>, GeoSpace2d> as Geometry>::Point,
        Disambig<LineString<C>, GeoSpace2d>,
        SimpleContourSymbol,
        GeoSpace2d,
    >,
    shortest_path_layer: FeatureLayer<
        <Disambig<LineString<C>, GeoSpace2d> as Geometry>::Point,
        Disambig<LineString<C>, GeoSpace2d>,
        SimpleContourSymbol,
        GeoSpace2d,
    >,
    detour_layer: FeatureLayer<
        <Disambig<LineString<C>, GeoSpace2d> as Geometry>::Point,
        Disambig<LineString<C>, GeoSpace2d>,
        SimpleContourSymbol,
        GeoSpace2d,
    >,
    point_layer: FeatureLayer<
        <Disambig<MultiPoint<C>, GeoSpace2d> as Geometry>::Point,
        Disambig<MultiPoint<C>, GeoSpace2d>,
        CirclePointSymbol,
        GeoSpace2d,
    >,
    poi_layer: FeatureLayer<
        <Disambig<Point<C>, GeoSpace2d> as Geometry>::Point,
        Disambig<Point<C>, GeoSpace2d>,
        CirclePointSymbol,
        GeoSpace2d,
    >,
}

impl<C> BlockPairLayer<C>
where
    C: RTreeNum + CoordFloat + FromPrimitive + maybe_sync::MaybeSync + maybe_sync::MaybeSend,
    Coord<C>: NewGeoPoint,
{
    pub fn new(crs: Crs) -> Self {
        Self {
            poly_layer: FeatureLayer::new(
                vec![],
                SimplePolygonSymbol {
                    fill_color: Color::TRANSPARENT,
                    stroke_color: Color::rgba(255, 0, 255, 255),
                    stroke_width: 2.,
                    stroke_offset: 0.,
                },
                crs.clone(),
            ),
            radius_layer: FeatureLayer::new(
                vec![],
                SimpleContourSymbol::new(Color::BLUE, 2.),
                crs.clone(),
            ),
            shortest_path_layer: FeatureLayer::new(
                vec![],
                SimpleContourSymbol::new(Color::GREEN, 2.),
                crs.clone(),
            ),
            detour_layer: FeatureLayer::new(
                vec![],
                SimpleContourSymbol::new(Color::RED, 2.),
                crs.clone(),
            ),
            point_layer: FeatureLayer::new(
                vec![],
                CirclePointSymbol::new(Color::GREEN, 6.),
                crs.clone(),
            ),

            poi_layer: FeatureLayer::new(
                vec![],
                CirclePointSymbol::new(Color::from_hex("#F7F304"), 6.),
                crs,
            ),
        }
    }
    pub fn show_block_pair<G, EV>(&mut self, block_pair: BlockPair<EV, C>, graph: &G)
    where
        G: CoordGraph<C = C, EV = EV> + Dijkstra,
        EV: FloatCore + Debug,
    {
        if let Some(s_repr) = graph.node_coord(block_pair.values().s)
            && let Some(t_repr) = graph.node_coord(block_pair.values().t)
        {
            let points = MultiPoint::new(vec![geo::Point::from(s_repr), geo::Point::from(t_repr)]);

            info!("inserting points {points:?}");

            let point_feats = self.point_layer.features_mut();
            let f_ids: Vec<_> = point_feats.iter_mut().map(|f| f.0).collect();

            for f_id in f_ids {
                point_feats.remove(f_id);
            }

            point_feats.add(points.to_geo2d());
            self.point_layer.update_all_features();
        }

        if let Some(poi) = graph.node_coord(block_pair.poi_id()) {
            let poi = geo::Point::from(poi);
            let poi_feats = self.poi_layer.features_mut();
            let f_ids: Vec<_> = poi_feats.iter_mut().map(|f| f.0).collect();

            for f_id in f_ids {
                poi_feats.remove(f_id);
            }

            poi_feats.add(poi.to_geo2d());
            self.poi_layer.update_all_features();
        }

        let s_paths = graph.dijkstra(
            block_pair.values().s,
            FxHashSet::from_iter([block_pair.poi_id(), block_pair.values().t]),
            graph_rs::types::Direction::Outgoing,
        );
        let p_paths = graph.dijkstra(
            block_pair.poi_id(),
            FxHashSet::from_iter([block_pair.values().t]),
            graph_rs::types::Direction::Outgoing,
        );

        let paths = [
            s_paths.path(block_pair.poi_id()).unwrap(),
            p_paths.path(block_pair.values().t).unwrap(),
        ];

        let shortest_path = s_paths
            .path(block_pair.values().t)
            .unwrap()
            .line_string(graph);
        {
            let shortest_path_feats = self.shortest_path_layer.features_mut();
            let f_ids: Vec<_> = shortest_path_feats.iter().map(|f| f.0).collect();

            for f_id in f_ids {
                shortest_path_feats.remove(f_id);
            }

            if let Some(path) = shortest_path {
                shortest_path_feats.add(path.to_geo2d());
            }

            self.shortest_path_layer.update_all_features();
        }

        let paths = paths.iter().map(|path| path.line_string(graph));

        {
            let detour_feats = self.detour_layer.features_mut();
            let f_ids: Vec<_> = detour_feats.iter().map(|f| f.0).collect();

            for f_id in f_ids {
                detour_feats.remove(f_id);
            }

            for path in paths.flatten() {
                detour_feats.add(path.to_geo2d());
            }

            self.detour_layer.update_all_features();
        }

        let lines = [
            &block_pair.values().r_af,
            &block_pair.values().r_ab,
            &block_pair.values().r_bf,
            &block_pair.values().r_bb,
        ];
        let lines = lines.iter().map(|path| path.line_string(graph));

        info!("inserting lines {lines:?}");

        {
            let line_feats = self.radius_layer.features_mut();
            let f_ids: Vec<_> = line_feats.iter_mut().map(|f| f.0).collect();

            for f_id in f_ids {
                line_feats.remove(f_id);
            }

            for line in lines.flatten() {
                line_feats.add(line.to_geo2d());
            }

            self.radius_layer.update_all_features();
        };

        let s_poly = block_pair.s_block().to_polygon();
        let t_poly = block_pair.t_block().to_polygon();

        let polys = geo_types::MultiPolygon::new(vec![s_poly, t_poly]);

        info!("inserting polygons {polys:?}");

        let feature_id = {
            let poly_feats = self.poly_layer.features_mut();
            let f_ids: Vec<_> = poly_feats.iter_mut().map(|f| f.0).collect();

            for f_id in f_ids {
                poly_feats.remove(f_id);
            }

            let f_id = poly_feats.add(polys.to_geo2d());
            self.poly_layer.update_all_features();

            f_id
        };
    }

    pub fn insert<EV>(&mut self, block_pair: BlockPair<EV, C>) -> FeatureId
    where
        EV: FloatCore + Debug,
    {
        let s_poly = block_pair.s_block().to_polygon();
        let t_poly = block_pair.t_block().to_polygon();

        let polys = geo_types::MultiPolygon::new(vec![s_poly, t_poly]);

        info!("inserting polygons {polys:?}");

        self.poly_layer.features_mut().add(polys.to_geo2d())
    }
}

impl<C> GalileoLayer for BlockPairLayer<C>
where
    C: RTreeNum
        + CoordFloat
        + FromPrimitive
        + Bounded
        + Scalar
        + maybe_sync::MaybeSend
        + maybe_sync::MaybeSync
        + 'static,
    Coord<C>: NewGeoPoint,
    Point<C>: NewGeoPoint,
{
    fn render(&self, view: &galileo::MapView, canvas: &mut dyn galileo::render::Canvas) {
        self.poly_layer.render(view, canvas);
        self.radius_layer.render(view, canvas);
        self.shortest_path_layer.render(view, canvas);
        self.detour_layer.render(view, canvas);
        self.point_layer.render(view, canvas);
        self.poi_layer.render(view, canvas);
    }

    fn prepare(&self, view: &galileo::MapView) {
        self.poly_layer.prepare(view);
        self.radius_layer.prepare(view);
        self.shortest_path_layer.prepare(view);
        self.detour_layer.prepare(view);
        self.point_layer.prepare(view);
        self.poi_layer.prepare(view);
    }

    fn set_messenger(&mut self, messenger: Box<dyn galileo::Messenger>) {
        let messenger = ArcMessenger(Arc::new(messenger));
        self.poly_layer.set_messenger(Box::new(messenger.clone()));
        self.radius_layer.set_messenger(Box::new(messenger.clone()));
        self.shortest_path_layer
            .set_messenger(Box::new(messenger.clone()));
        self.detour_layer.set_messenger(Box::new(messenger.clone()));
        self.point_layer.set_messenger(Box::new(messenger.clone()));
        self.poi_layer.set_messenger(Box::new(messenger));
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

impl<C> EventLayer for BlockPairLayer<C>
where
    C: RTreeNum
        + CoordFloat
        + FromPrimitive
        + Bounded
        + Scalar
        + maybe_sync::MaybeSend
        + maybe_sync::MaybeSync
        + 'static,
    Coord<C>: NewGeoPoint,
    Point<C>: NewGeoPoint,
{
    fn handle_event(&self, event: &galileo::control::UserEvent, map: &mut galileo::Map) {}
}

#[derive(Clone)]
struct ArcMessenger(Arc<Box<dyn galileo::Messenger>>);

impl galileo::Messenger for ArcMessenger {
    fn request_redraw(&self) {
        self.0.request_redraw();
    }
}
