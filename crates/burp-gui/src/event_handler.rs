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
    OracleBuild(Oracle<f64, f64>, Tree<BlockPair<f64, f64>>),
}

impl Event {
    #[instrument(skip(app_data))]
    pub fn handle(self, app_data: &mut AppData) {
        match self {
            Self::GraphLoaded(graph) => {
                app_data.graph = Some(Arc::new(RwLock::new(Dirty::new(graph))));
                log::debug!("Processed event GraphLoaded");
            }
            Self::OracleBuild(oracle, split_tree) => {
                app_data.oracle = Some(Arc::new(Mutex::new(Dirty::new(oracle))));
                app_data.split_tree = Some(Arc::new(RwLock::new(Dirty::new(split_tree))));
                log::info!("Processed event OracleBuild");
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

        log::trace!("[event] Recieved {} events", events.len());

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
