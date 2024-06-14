use crate::input::edgelist::EdgeList;

pub struct Uninitialized {}

pub struct FromEdgeList {
    edges: EdgeList<usize>,
}

#[derive(Debug)]
pub struct GraphBuilder<State> {
    state: State,
}

impl GraphBuilder<Uninitialized> {
    pub fn new() -> Self {
        Self {
            state: Uninitialized {},
        }
    }

    pub fn string(self, s: String) -> GraphBuilder<FromEdgeList> {
        GraphBuilder {
            state: FromEdgeList {
                edges: EdgeList::try_from(&s).unwrap(),
            },
        }
    }
}

impl GraphBuilder<FromEdgeList> {
    pub fn build<DirectedGraph>(self) -> DirectedGraph
    where
        DirectedGraph: From<EdgeList<usize>>,
    {
        DirectedGraph::from(self.state.edges)
    }
}
