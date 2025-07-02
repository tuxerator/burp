use burp::types::Poi;
use galileo::{
    error::GalileoError,
    symbol::{ImagePointSymbol, Symbol},
};
use galileo_types::geometry::Geom;
use nalgebra::Vector2;
use num_traits::AsPrimitive;

use crate::map::layers::node_layer::NodeMarker;

pub struct PoiSymbol {
    image_symbol: ImagePointSymbol,
}

impl PoiSymbol {
    pub fn from_path(path: &str, offset: Vector2<f32>, scale: f32) -> Result<Self, GalileoError> {
        let image_symbol = ImagePointSymbol::from_path(path, offset, scale)?;
        Ok(Self { image_symbol })
    }
}

impl Symbol<NodeMarker<Poi>> for PoiSymbol {
    fn render<'a, N, P>(
        &self,
        feature: &NodeMarker<Poi>,
        geometry: &'a galileo_types::geometry::Geom<P>,
        min_resolution: f64,
    ) -> Vec<
        galileo::render::render_bundle::RenderPrimitive<
            'a,
            N,
            P,
            galileo_types::impls::Contour<P>,
            galileo_types::impls::Polygon<P>,
        >,
    >
    where
        N: AsPrimitive<f32>,
        P: galileo_types::cartesian::CartesianPoint3d<Num = N> + Clone,
    {
        if feature.data().is_none() {
            return vec![];
        }
        match geometry {
            Geom::Point(point) => self.image_symbol.render(feature, geometry, min_resolution),
            Geom::MultiPoint(points) => self.image_symbol.render(feature, geometry, min_resolution),
            _ => vec![],
        }
    }
}

pub struct BlockPairSymbol;
