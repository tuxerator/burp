use std::sync::Arc;
use std::{fmt::Display, sync::RwLock};

use burp::tree::Tree;
use burp::tree::node::Node;
use egui::{CentralPanel, CollapsingHeader, Ui, Widget};

use crate::map::Map;
use crate::map::layers::block_pair_layer::BlockPairLayer;

pub struct TreeView<'a, T: Display, F>
where
    F: FnMut(&mut Ui, &Node<T>, (usize, usize)),
{
    tree: &'a Tree<T>,
    node_ui: F,
}

impl<'a, T: Display, F> TreeView<'a, T, F>
where
    F: FnMut(&mut Ui, &Node<T>, (usize, usize)),
{
    pub fn new(tree: &'a Tree<T>, node_ui: F) -> Self {
        Self { tree, node_ui }
    }
}

impl<T: Display, F> Widget for TreeView<'_, T, F>
where
    F: FnMut(&mut Ui, &Node<T>, (usize, usize)),
{
    fn ui(mut self, ui: &mut egui::Ui) -> egui::Response {
        egui::ScrollArea::vertical().show(ui, |ui| {
            build_tree_view(ui, self.tree.get_root(), &mut self.node_ui, (0, 0));
        });

        ui.response()
    }
}

fn build_tree_view<T: Display>(
    ui: &mut egui::Ui,
    root: &Node<T>,
    node_ui: &mut dyn FnMut(&mut Ui, &Node<T>, (usize, usize)),
    id: (usize, usize),
) {
    node_ui(ui, root, id);

    if let Some(children) = root.get_children() {
        let childen = CollapsingHeader::new("Children").id_salt((id, 1));
        childen.show(ui, |ui| {
            let mut i = 0;
            for child in children {
                build_tree_view(ui, child, node_ui, (id.0 + 1, id.1 + i));
                i += 1;
            }
        });
    }
}
