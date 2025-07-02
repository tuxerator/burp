use std::sync::Arc;

use burp::oracle::block_pair::BlockPair;
use galileo::{
    Color, Messenger,
    layer::{FeatureLayer, Layer as GalileoLayer, feature_layer::Feature},
    symbol::{
        ArbitraryGeometrySymbol, CirclePointSymbol, SimpleContourSymbol, SimplePolygonSymbol,
    },
};
use galileo_types::{
    Disambig, Geometry, MultiPolygon as MultiPolygonTrait,
    cartesian::{CartesianPoint2d, NewCartesianPoint2d},
    geo::{Crs, Datum, NewGeoPoint, impls::projection::WebMercator},
    geometry::Geom,
    geometry_type::CartesianSpace2d,
    impls::MultiPolygon,
};
use geo::{Coord, CoordFloat, LineString, Polygon};
use log::info;
use nalgebra::Scalar;
use num_traits::{Bounded, FromPrimitive, float::FloatCore};
use rstar::RTreeNum;

use crate::map::layers::EventLayer;

pub struct BlockPairLayer<C>
where
    C: RTreeNum + CoordFloat + FromPrimitive + 'static,
{
    poly_layer:
        FeatureLayer<Coord<C>, MultiPolygon<Coord<C>>, SimplePolygonSymbol, CartesianSpace2d>,
    line_layer: FeatureLayer<
        <Disambig<LineString<C>, CartesianSpace2d> as Geometry>::Point,
        Disambig<LineString<C>, CartesianSpace2d>,
        SimpleContourSymbol,
        CartesianSpace2d,
    >,
    point_layer: FeatureLayer<
        <Disambig<Coord<C>, CartesianSpace2d> as Geometry>::Point,
        Disambig<Coord<C>, CartesianSpace2d>,
        CirclePointSymbol,
        CartesianSpace2d,
    >,
}

impl<C> BlockPairLayer<C>
where
    C: RTreeNum + CoordFloat + FromPrimitive,
    Coord<C>: NewGeoPoint + NewCartesianPoint2d,
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
            line_layer: FeatureLayer::new(
                vec![],
                SimpleContourSymbol::new(Color::BLUE, 2.),
                crs.clone(),
            ),
            point_layer: FeatureLayer::new(vec![], CirclePointSymbol::new(Color::RED, 2.), crs),
        }
    }
    pub fn show_block_pair<EV>(&mut self, block_pair: BlockPair<EV, C>)
    where
        EV: FloatCore,
    {
        let projection: WebMercator<Coord<C>, Coord<C>> = WebMercator::new(Datum::WGS84);
        let s_poly = block_pair.s_block().to_polygon();
        let t_poly = block_pair.t_block().to_polygon();

        let polys = geo_types::MultiPolygon::new(vec![s_poly, t_poly]);

        info!("inserting polygons {polys:?}");

        let polys = polys.project(&projection).unwrap();
        let Geom::MultiPolygon(polys) = polys else {
            return;
        };
        let features = self.poly_layer.features_mut();
        features.remove_all();
        features.insert(polys);
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
    Coord<C>: NewCartesianPoint2d,
{
    fn render(&self, view: &galileo::MapView, canvas: &mut dyn galileo::render::Canvas) {
        self.poly_layer.render(view, canvas);
        self.line_layer.render(view, canvas);
        self.point_layer.render(view, canvas);
    }

    fn prepare(&self, view: &galileo::MapView) {
        self.poly_layer.prepare(view);
        self.line_layer.prepare(view);
        self.point_layer.prepare(view);
    }

    fn set_messenger(&mut self, messenger: Box<dyn galileo::Messenger>) {
        // let messenger = ArcMessenger(Arc::new(messenger));
        self.poly_layer.set_messenger(messenger);
        // self.line_layer.set_messenger(Box::new(messenger.clone()));
        // self.point_layer.set_messenger(Box::new(messenger));
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
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
    Coord<C>: NewCartesianPoint2d,
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
