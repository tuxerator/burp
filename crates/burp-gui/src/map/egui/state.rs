use egui::{Context, Id, Response};
use galileo_types::geo::impls::GeoPoint2d;

#[derive(Clone, Debug)]
pub struct MapState {
    pub clicked: bool,
    pub map_center_pos: Option<GeoPoint2d>,
    pub map_interact_pos: Option<GeoPoint2d>,
}

impl MapState {
    pub fn load(ctx: &Context, id: Id) -> Self {
        ctx.data_mut(|d| d.get_temp(id).unwrap_or_default())
    }
    pub fn save(self, ctx: &Context, id: Id) {
        ctx.data_mut(|d| d.insert_temp(id, self))
    }
}

impl Default for MapState {
    fn default() -> Self {
        Self {
            clicked: false,
            map_center_pos: None,
            map_interact_pos: None,
        }
    }
}
