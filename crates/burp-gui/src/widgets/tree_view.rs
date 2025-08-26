use std::sync::Arc;
use std::{fmt::Display, sync::RwLock};

use egui::{CentralPanel, CollapsingHeader, Ui, Widget};
use id_tree::{Node, NodeId, Tree};

use crate::map::Map;
use crate::map::layers::block_pair_layer::BlockPairLayer;

pub struct TreeView<'a, T, F>
where
    F: FnMut(&mut Ui, &Node<T>, (usize, usize)),
{
    tree: &'a Tree<T>,
    node_ui: F,
}

impl<'a, T, F> TreeView<'a, T, F>
where
    F: FnMut(&mut Ui, &Node<T>, (usize, usize)),
{
    pub fn new(tree: &'a Tree<T>, node_ui: F) -> Self {
        Self { tree, node_ui }
    }

    fn build_tree_view(&mut self, ui: &mut egui::Ui, root: &NodeId, id: (usize, usize)) {
        (self.node_ui)(ui, self.tree.get(root).unwrap(), id);

        let mut children = self.tree.children_ids(root).unwrap().peekable();

        if children.peek().is_some() {
            let childen = CollapsingHeader::new("Children").id_salt((id, 1));
            childen.show(ui, |ui| {
                let mut i = 0;
                for child in children {
                    self.build_tree_view(ui, child, (id.0 + 1, id.1 + i));
                    i += 1;
                }
            });
        }
    }
}

impl<T, F> Widget for TreeView<'_, T, F>
where
    F: FnMut(&mut Ui, &Node<T>, (usize, usize)),
{
    fn ui(mut self, ui: &mut egui::Ui) -> egui::Response {
        egui::ScrollArea::vertical().show(ui, |ui| {
            self.build_tree_view(ui, self.tree.root_node_id().unwrap(), (0, 0));
        });

        ui.response()
    }
}
