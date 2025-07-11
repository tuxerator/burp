use burp::types::Poi;
use galileo::{
    error::GalileoError,
    symbol::{ImagePointSymbol, Symbol},
};
use galileo_types::geometry::Geom;
use num_traits::AsPrimitive;

use crate::map::layers::node_layer::NodeMarker;

pub struct PoiSymbol {
    image_symbol: ImagePointSymbol,
}

impl PoiSymbol {
    pub fn from_path(
        path: &str,
        offset: galileo_types::cartesian::Vector2<f32>,
        scale: f32,
    ) -> Result<Self, GalileoError> {
        let image_symbol = ImagePointSymbol::from_path(path, offset, scale)?;
        Ok(Self { image_symbol })
    }
}

impl Symbol<NodeMarker<Poi>> for PoiSymbol {
    fn render(
        &self,
        feature: &NodeMarker<Poi>,
        geometry: &Geom<galileo_types::cartesian::Point3>,
        min_resolution: f64,
        bundle: &mut galileo::render::render_bundle::RenderBundle,
    ) {
        let Some(data) = feature.data() else {
            return;
        };

        self.image_symbol
            .render(feature, geometry, min_resolution, bundle);
    }
}

pub struct BlockPairSymbol;
