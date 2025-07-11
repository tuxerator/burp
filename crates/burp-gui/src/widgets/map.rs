use std::hash::Hash;

use egui::Widget;

use crate::map::egui_state::EguiMapState;

pub struct Map<'map, K>
where
    K: Hash + Eq,
{
    map_state: &'map mut EguiMapState<K>,
}

impl<'map, K> Map<'map, K>
where
    K: Hash + Eq,
{
    pub fn new(map_state: &'map mut EguiMapState<K>) -> Self {
        Self { map_state }
    }
}

impl<'map, K> Widget for Map<'map, K>
where
    K: Hash + Eq,
{
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.map_state.render(ui)
    }
}
