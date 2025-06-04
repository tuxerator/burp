use log::{error, info};

use crate::DirectedGraph;

#[derive(Clone, Copy, Debug)]
struct NodeData {
    rootindex: Option<usize>,
}

struct TarjanSCCData {
    index: usize,
    nodes: Vec<NodeData>,
    stack: Vec<usize>,
    sccs: Vec<Vec<usize>>,
}

impl TarjanSCCData {
    fn new() -> Self {
        TarjanSCCData {
            index: 0,
            nodes: Vec::new(),
            stack: Vec::new(),
            sccs: Vec::new(),
        }
    }

    fn run<G: DirectedGraph>(&mut self, g: &G) {
        info!("SCC for graph with {} nodes", g.node_count());
        self.nodes.clear();
        self.nodes
            .resize(g.node_count(), NodeData { rootindex: None });

        for v in 0..g.node_count() {
            let visited = self.nodes[v].rootindex.is_some();
            if !visited {
                self.visit(g, v);
            }
        }
    }

    fn visit<G: DirectedGraph>(&mut self, g: &G, v: usize) {
        let node_v = &mut self.nodes[v];
        let mut v_is_local_root = true;
        node_v.rootindex = Some(self.index);
        self.index += 1;
        self.stack.push(v);

        for w in g.out_neighbors(v) {
            if self.nodes[w.target()].rootindex.is_none() {
                self.visit(g, w.target())
            }

            if self.nodes[w.target()].rootindex < self.nodes[v].rootindex {
                self.nodes[v].rootindex = self.nodes[w.target()].rootindex;
                v_is_local_root = false;
            }
        }

        if v_is_local_root {
            let mut scc = Vec::new();
            while self
                .stack
                .last()
                .is_some_and(|w| self.nodes[*w].rootindex >= self.nodes[v].rootindex)
            {
                scc.push(self.stack.pop().expect("stack is empty"));
            }

            self.sccs.push(scc);
        }
    }
}
pub trait TarjanSCC {
    fn tarjan_scc(&self) -> Vec<Vec<usize>>;
}

impl<G> TarjanSCC for G
where
    G: DirectedGraph,
{
    fn tarjan_scc(&self) -> Vec<Vec<usize>> {
        let mut tarjan_scc = TarjanSCCData::new();

        tarjan_scc.run(self);

        tarjan_scc.sccs
    }
}

#[cfg(test)]
mod test {
    use crate::{graph::csr::DirectedCsrGraph, input::edgelist::EdgeList};

    use super::TarjanSCC;

    #[test]
    fn tarjan() {
        let edges = EdgeList::new(vec![
            (0, 3, 0),
            (0, 5, 0),
            (1, 0, 0),
            (1, 5, 0),
            (2, 4, 0),
            (3, 0, 0),
            (3, 2, 0),
            (4, 1, 0),
        ]);

        let csr = DirectedCsrGraph::from(edges);

        let sccs_expected = vec![vec![5], vec![0, 1, 2, 3, 4]];

        let mut sccs_actual = csr.tarjan_scc();

        sccs_actual.iter_mut().for_each(|scc| scc.sort());

        sccs_actual.sort_by(|lhs, rhs| lhs.len().cmp(&rhs.len()));

        assert_eq!(sccs_actual, sccs_expected);
    }
}
