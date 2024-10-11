use std::{f32, f64, sync::Mutex};

use galileo::{
    control::{EventPropagation, UserEvent, UserEventHandler},
    layer::{feature_layer::Feature, FeatureLayer, Layer as GalileoLayer},
    render::{render_bundle::RenderPrimitive, LineCap, LinePaint},
    symbol::{SimplePolygonSymbol, Symbol},
    Color, Map,
};
use galileo_types::{
    cartesian::{
        CartesianPoint2d, CartesianPoint3d, NewCartesianPoint2d, NewCartesianPoint3d, Point2d,
    },
    geo::{
        impls::{
            projection::{self, WebMercator},
            GeoPoint2d,
        },
        Crs, Datum,
    },
    geometry::{Geom, Geometry},
    geometry_type::CartesianSpace2d,
    impls::{Contour, MultiPolygon, Polygon as GalileoPolygon},
    Disambig, Disambiguate, MultiPolygon as MultiPolygonTrait,
};
use geo::{Centroid, LineString};
use geo_types::geometry::{Coord, Polygon};
use graph_rs::{CoordGraph, Coordinate};
use maybe_sync::{MaybeSend, MaybeSync};
use nalgebra::{Point3, Scalar, Vector2};
use num_traits::{AsPrimitive, Bounded, FromPrimitive, Num, ToPrimitive};

use super::EventLayer;

pub struct BlocksLayer<S>
where
    S: Symbol<Blocks>,
{
    layer: Mutex<
        FeatureLayer<
            <Disambig<Coord, CartesianSpace2d> as Geometry>::Point,
            Blocks,
            S,
            CartesianSpace2d,
        >,
    >,
    shown_features: Mutex<Vec<usize>>,
    color_id: u8,
}

impl<S> BlocksLayer<S>
where
    S: Symbol<Blocks>,
{
    pub fn new(style: S) -> Self {
        Self {
            layer: Mutex::new(FeatureLayer::with_lods(
                vec![],
                style,
                Crs::EPSG3857,
                &[8000.0, 1000.0, 1.0],
            )),
            shown_features: Mutex::new(vec![]),
            color_id: 0,
        }
    }

    pub fn insert_block_pair(&mut self, pair: (Polygon, Polygon)) {
        let projection: WebMercator<Coord, Disambig<Coord, CartesianSpace2d>> =
            WebMercator::new(Datum::WGS84);
        let mut line = LineString::new(vec![
            pair.0.centroid().unwrap().as_coord(),
            pair.1.centroid().unwrap().as_coord(),
        ]);
        line.close();
        let line = line.project(&projection).unwrap();
        let pair = (
            pair.0.project(&projection).unwrap(),
            pair.1.project(&projection).unwrap(),
        );

        let (Geom::Polygon(poly_a), Geom::Polygon(poly_b)) = pair else {
            return;
        };

        let Geom::Contour(line) = line else {
            return;
        };

        let multi_poly = MultiPolygon::from(vec![
            poly_a,
            poly_b,
            GalileoPolygon::new(line.into_closed().unwrap(), vec![]),
        ]);

        let block_pair = Blocks {
            multi_poly,
            color: self.get_color(),
        };

        self.layer
            .lock()
            .unwrap()
            .features_mut()
            .insert_hidden(block_pair);
    }

    fn get_color(&mut self) -> Color {
        let red = (((2.0 * f32::consts::PI) * (self.color_id.to_f32().unwrap() / 1.0)).sin()
            * 127.0
            + 128.0)
            .to_u8()
            .unwrap();
        let green = (((2.0 * f32::consts::PI + 2.0) * (self.color_id.to_f32().unwrap() / 1.0))
            .sin()
            * 127.0
            + 128.0)
            .to_u8()
            .unwrap();
        let blue = (((2.0 * f32::consts::PI + 4.0) * (self.color_id.to_f32().unwrap() / 1.0))
            .sin()
            * 127.0
            + 128.0)
            .to_u8()
            .unwrap();

        self.color_id = if self.color_id < 31 {
            self.color_id + 1
        } else {
            0
        };
        Color::rgba(red, green, blue, 255)
    }
}

impl<S> GalileoLayer for BlocksLayer<S>
where
    S: Symbol<Blocks> + MaybeSend + MaybeSync + 'static,
{
    fn render(&self, view: &galileo::MapView, canvas: &mut dyn galileo::render::Canvas) {
        self.layer.lock().unwrap().render(view, canvas)
    }

    fn prepare(&self, view: &galileo::MapView) {
        self.layer.lock().unwrap().prepare(view)
    }

    fn set_messenger(&mut self, messenger: Box<dyn galileo::Messenger>) {
        self.layer.lock().unwrap().set_messenger(messenger)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl<S> EventLayer for BlocksLayer<S>
where
    S: Symbol<Blocks> + MaybeSend + MaybeSync + 'static,
{
    fn handle_event(&self, event: &UserEvent, map: &mut Map) {
        match event {
            UserEvent::Click(galileo::control::MouseButton::Left, mouse_event) => {
                let Some(position) = map
                    .view()
                    .screen_to_map(mouse_event.screen_pointer_position)
                else {
                    return;
                };

                let mut layer = self.layer.lock().unwrap();

                layer.features_mut().iter_mut().for_each(|mut f| f.hide());

                let features = layer.get_features_at_mut(&position, 0.0);

                for mut feature in features {
                    feature.show();
                }
            }
            _ => (),
        }
    }
}

impl<S> UserEventHandler for BlocksLayer<S>
where
    S: Symbol<Blocks> + MaybeSend + MaybeSync + 'static,
{
    fn handle(&self, event: &UserEvent, map: &mut Map) -> EventPropagation {
        self.handle_event(event, map);
        EventPropagation::Propagate
    }
}

pub struct Blocks {
    pub multi_poly: MultiPolygon<Disambig<Coord, CartesianSpace2d>>,
    pub color: Color,
}

impl Feature for Blocks {
    type Geom = MultiPolygon<Disambig<Coord, CartesianSpace2d>>;
    fn geometry(&self) -> &Self::Geom {
        &self.multi_poly
    }
}

pub struct BlocksSymbol {}

impl BlocksSymbol {
    pub fn new() -> Self {
        Self {}
    }

    fn get_polygon_symbol(&self, feature: &Blocks) -> SimplePolygonSymbol {
        let stroke_color = feature.color;

        SimplePolygonSymbol {
            stroke_color,
            fill_color: Color::TRANSPARENT,
            stroke_width: 2.0,
            stroke_offset: 0.0,
        }
    }

    fn shift_color(&self) {}
}

impl Symbol<Blocks> for BlocksSymbol {
    fn render<'a, N, P>(
        &self,
        feature: &Blocks,
        geometry: &'a Geom<P>,
        min_resolution: f64,
    ) -> Vec<
        galileo::render::render_bundle::RenderPrimitive<
            'a,
            N,
            P,
            Contour<P>,
            galileo_types::impls::Polygon<P>,
        >,
    >
    where
        N: num_traits::AsPrimitive<f32>,
        P: galileo_types::cartesian::CartesianPoint3d<Num = N> + Clone,
    {
        self.get_polygon_symbol(feature)
            .render(&(), geometry, min_resolution)
    }
}

fn center<P, N>(points: Vec<P>) -> Option<Point3<f32>>
where
    P: CartesianPoint3d<Num = N>,
    N: num_traits::AsPrimitive<f32>,
{
    let n: f32 = points.len().as_();
    let mut points_iter = points.into_iter();
    let first = points_iter.next()?;
    let first = Point3::new(first.x().as_(), first.y().as_(), first.z().as_());
    let point_sum = points_iter.fold(first, |acc, point| {
        Point3::new(
            acc.x() + point.x().as_(),
            acc.y() + point.y().as_(),
            acc.z() + point.z().as_(),
        )
    });

    Some(Point3::new(
        point_sum.x() / n,
        point_sum.y() / n,
        point_sum.z() / n,
    ))
}
