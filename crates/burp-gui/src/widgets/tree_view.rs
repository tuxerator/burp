use std::sync::Arc;
use std::{fmt::Display, sync::RwLock};

use burp::tree::Tree;
use burp::tree::node::Node;
use egui::{CentralPanel, CollapsingHeader, Ui, Widget};

use crate::map::Map;
use crate::map::layers::block_pair_layer::BlockPairLayer;

pub struct TreeView<'a, T: Display, F>
where
    F: Fn(&mut Ui, &Node<T>, (usize, usize)),
{
    tree: &'a Tree<T>,
    node_ui: F,
}

impl<'a, T: Display, F> TreeView<'a, T, F>
where
    F: Fn(&mut Ui, &Node<T>, (usize, usize)),
{
    pub fn new(tree: &'a Tree<T>, node_ui: F) -> Self {
        Self { tree, node_ui }
    }
}

impl<T: Display, F> Widget for TreeView<'_, T, F>
where
    F: Fn(&mut Ui, &Node<T>, (usize, usize)),
{
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        egui::ScrollArea::vertical()
            .show(ui, |ui| {
                build_tree_view(ui, self.tree.get_root(), &self.node_ui, (0, 0)).header_response
            })
            .inner
    }
}

fn build_tree_view<T: Display>(
    ui: &mut egui::Ui,
    root: &Node<T>,
    node_ui: &dyn Fn(&mut Ui, &Node<T>, (usize, usize)),
    id: (usize, usize),
) -> egui::CollapsingResponse<()> {
    ui.label("BlockPair");

    node_ui(ui, root, id);

    let values = CollapsingHeader::new("Values").id_source((id, 0));
    let childen = CollapsingHeader::new("Children").id_source((id, 1));
    values.show(ui, |ui| ui.label(format!("{}", root.get_data())));
    childen.show(ui, |ui| {
        let mut i = 0;
        if let Some(children) = root.get_children() {
            for child in children {
                build_tree_view(ui, child, node_ui, (id.0 + 1, id.1 + i));
                i += 1;
            }
        }
    })
}
