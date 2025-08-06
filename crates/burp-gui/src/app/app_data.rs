use std::{
    fmt::Display,
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::Arc,
};

use burp::{
    oracle::{PoiGraph, block_pair::BlockPair, oracle::Oracle},
    tree::Tree,
    types::Poi,
};
use memmap2::MmapOptions;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize, ser::SerializeStruct};

use crate::types::Dirty;

#[derive(Clone, Default, Deserialize)]
#[serde(from = "AppDataSerde")]
pub struct AppData {
    pub(crate) graph: Option<Arc<RwLock<Dirty<PoiGraph<Poi>>>>>,
    pub(crate) oracle: Option<Arc<Mutex<Dirty<Oracle<f64, f64>>>>>,
    pub(crate) split_tree: Option<Arc<RwLock<Dirty<Tree<BlockPair<f64, f64>>>>>>,
}

impl AppData {
    pub fn load_from_path(path: PathBuf) -> Self {
        let mut graph_path = path.clone();
        graph_path.push("graph.mp");

        let mut oracle_path = path.clone();
        oracle_path.push("oracle.mp");

        let mut split_tree_path = path.clone();
        split_tree_path.push("split_tree.mp");

        Self {
            graph: if let Ok(graph_file) = std::fs::File::open(graph_path) {
                tracing::debug!("Loading graph from \'{:?}\'", graph_file);
                let graph_file_mmap = unsafe { MmapOptions::new().map(&graph_file).unwrap() };
                let graph = rmp_serde::from_read(graph_file_mmap.as_ref()).ok();
                if graph.is_some() {
                    tracing::info!("Loaded graph from file");
                } else {
                    tracing::warn!("Failed to load graph")
                }

                graph.map(|graph| Arc::new(RwLock::new(Dirty::new(graph))))
            } else {
                None
            },

            oracle: if let Ok(oracle_file) = std::fs::File::open(oracle_path) {
                tracing::debug!("Loading oracle from \'{:?}\'", oracle_file);
                let oracle_file_mmap = unsafe { MmapOptions::new().map(&oracle_file).unwrap() };
                let oracle = rmp_serde::from_read(oracle_file_mmap.as_ref()).ok();
                if oracle.is_some() {
                    tracing::info!("Loaded oracle from file");
                } else {
                    tracing::warn!("Failed to load oracle")
                }

                oracle.map(|oracle| Arc::new(Mutex::new(Dirty::new(oracle))))
            } else {
                None
            },

            split_tree: if let Ok(split_tree_file) = std::fs::File::open(split_tree_path) {
                tracing::debug!("Loading split_tree from \'{:?}\'", split_tree_file);
                let split_tree_file_mmap =
                    unsafe { MmapOptions::new().map(&split_tree_file).unwrap() };
                let split_tree = rmp_serde::from_read(split_tree_file_mmap.as_ref()).ok();
                if split_tree.is_some() {
                    tracing::info!("Loaded split_tree from file");
                } else {
                    tracing::warn!("Failed to load split_tree")
                }

                split_tree.map(|split_tree| Arc::new(RwLock::new(Dirty::new(split_tree))))
            } else {
                None
            },
        }
    }
}

impl Serialize for AppData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("AppDataSerde", 3)?;

        if let Some(graph) = self.graph.as_ref().map(|g| g.read_arc()) {
            state.serialize_field("graph", &Some(graph.deref().deref()))?;
        } else {
            state.serialize_field("graph", &None::<PoiGraph<Poi>>)?;
        }

        if let Some(oracle) = self.oracle.as_ref().map(|o| o.lock_arc()) {
            state.serialize_field("oracle", &Some(oracle.deref()))?;
        } else {
            state.serialize_field("oracle", &None::<Oracle<f64, f64>>)?;
        }

        if let Some(split_tree) = self.split_tree.as_ref().map(|t| t.read_arc()) {
            state.serialize_field("split_tree", &Some(split_tree.deref()))?;
        } else {
            state.serialize_field("split_tree", &None::<Tree<BlockPair<f64, f64>>>)?;
        }

        state.end()
    }
}

#[derive(Serialize, Deserialize)]
struct AppDataSerde {
    graph: Option<PoiGraph<Poi>>,
    oracle: Option<Oracle<f64, f64>>,
    split_tree: Option<Tree<BlockPair<f64, f64>>>,
}

impl From<AppDataSerde> for AppData {
    fn from(value: AppDataSerde) -> Self {
        AppData {
            graph: value
                .graph
                .map(|graph| Arc::new(RwLock::new(Dirty::new(graph)))),
            oracle: value
                .oracle
                .map(|oracle| Arc::new(Mutex::new(Dirty::new(oracle)))),
            split_tree: value
                .split_tree
                .map(|split_tree| Arc::new(RwLock::new(Dirty::new(split_tree)))),
        }
    }
}

impl TryFrom<AppData> for AppDataSerde {
    type Error = FromAppDataError;
    fn try_from(value: AppData) -> Result<AppDataSerde, FromAppDataError> {
        Ok(Self {
            graph: match value.graph {
                Some(graph) => Some(
                    Arc::into_inner(graph)
                        .ok_or(FromAppDataError::GraphUnwrapError)?
                        .into_inner()
                        .into_inner(),
                ),
                None => None,
            },
            oracle: match value.oracle {
                Some(oracle) => Some(
                    Arc::into_inner(oracle)
                        .ok_or(FromAppDataError::OracleUnwrapError)?
                        .into_inner()
                        .into_inner(),
                ),
                None => None,
            },
            split_tree: match value.split_tree {
                Some(split_tree) => Some(
                    Arc::into_inner(split_tree)
                        .ok_or(FromAppDataError::SplitTreeUnwrapError)?
                        .into_inner()
                        .into_inner(),
                ),
                None => None,
            },
        })
    }
}

#[derive(Debug)]
enum FromAppDataError {
    GraphUnwrapError,
    OracleUnwrapError,
    SplitTreeUnwrapError,
}

impl std::error::Error for FromAppDataError {}

impl Display for FromAppDataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "more than one strong reference to {}",
            match self {
                FromAppDataError::GraphUnwrapError => "graph",
                FromAppDataError::OracleUnwrapError => "oracle",
                FromAppDataError::SplitTreeUnwrapError => "split_tree",
            }
        )
    }
}
