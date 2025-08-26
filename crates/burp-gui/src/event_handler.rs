use std::{
    fmt::{Debug, Display},
    sync::Arc,
};

use burp::{
    oracle::{PoiGraph, block_pair::BlockPair, oracle::Oracle},
    tree::Tree,
    types::Poi,
};
use parking_lot::{RwLock, lock_api::Mutex};
use tokio::{runtime::Runtime, sync::mpsc::error::TryRecvError, task::LocalSet};
use tracing::{field::DisplayValue, instrument};

use crate::{BurpApp, app::AppData, types::Dirty};

pub enum Event {
    GraphLoaded(PoiGraph<Poi>),
    OracleLoaded(Oracle<f64, f64>),
    SplitTreeLoaded((usize, id_tree::Tree<(BlockPair<f64, f64>, bool)>)),
    OracleBuild(Oracle<f64, f64>, id_tree::Tree<(BlockPair<f64, f64>, bool)>),
}

impl Event {
    #[instrument(skip(app_data))]
    pub fn handle(self, app_data: &mut AppData) {
        tracing::info!("Handling event");
        match self {
            Self::GraphLoaded(graph) => {
                app_data.graph = Some(Arc::new(RwLock::new(Dirty::new(graph))));
            }
            Self::OracleLoaded(oracle) => {
                app_data
                    .oracle
                    .get_or_insert_default()
                    .lock()
                    .insert(oracle);
            }
            Self::SplitTreeLoaded(split_tree) => {
                app_data
                    .split_tree
                    .get_or_insert_default()
                    .write()
                    .insert(split_tree.0, split_tree.1);
            }
            Self::OracleBuild(oracle, split_tree) => {
                app_data
                    .split_tree
                    .get_or_insert_default()
                    .write()
                    .insert(oracle.poi(), split_tree);
                app_data
                    .oracle
                    .get_or_insert_default()
                    .lock()
                    .insert(oracle);
            }
        }
    }
}

impl Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::GraphLoaded(_) => "GraphLoaded",
                Self::OracleLoaded(_) => "OracleLoaded",
                Self::SplitTreeLoaded(_) => "SplitTreeLoaded",
                Self::OracleBuild(_, _) => "OracleBuild",
            }
        )
    }
}

impl Debug for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::GraphLoaded(_) => "Event::GraphLoaded",
                Self::OracleLoaded(_) => "Event::OracleLoaded",
                Self::SplitTreeLoaded(_) => "Event::SplitTreeLoaded",
                Self::OracleBuild(_, _) => "Event::OracleBuild",
            }
        )
    }
}

pub struct EventHandler {
    recv: tokio::sync::mpsc::Receiver<Event>,
}

impl EventHandler {
    pub fn new(recv: tokio::sync::mpsc::Receiver<Event>) -> Self {
        Self { recv }
    }

    pub fn handle_events(&mut self, app_data: &mut AppData) {
        let mut events = Vec::with_capacity(self.recv.len());

        while !self.recv.is_empty() {
            events.push(match self.recv.try_recv() {
                Ok(v) => v,
                Err(err) => match err {
                    TryRecvError::Empty => return,
                    TryRecvError::Disconnected => {
                        panic!("All sender disconnected from event channel.")
                    }
                },
            });
        }

        log::trace!("Recieved {} events", events.len());

        for event in events {
            event.handle(app_data);
        }
    }
}

#[derive(Debug)]
pub struct EventHandlerError;

impl std::error::Error for EventHandlerError {}

impl std::fmt::Display for EventHandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Event channel closed")
    }
}
