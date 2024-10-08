use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    fmt::Debug,
    iter::Peekable,
};

use graph_rs::{
    algorithms::dijkstra::Dijkstra, graph::quad_tree::QuadGraph, types::Direction, Coordinate,
    DirectedGraph, Graph,
};
use indicatif::ProgressBar;
use log::{info, warn};
use ordered_float::FloatCore;
use qutee::{Boundary, DynCap, Point, QueryPoints};

pub struct Oracle {
    b_tree: BTreeMap<usize, usize>,
}

impl Oracle {
    fn new() {}
}

pub fn build<EV, NV, G>(
    graph: &QuadGraph<EV, NV, G>,
    poi: usize,
    epsilon: EV,
) -> Vec<(Boundary<f64>, Boundary<f64>)>
where
    EV: FloatCore + Default + Debug + Copy + Send + Sync,
    NV: Coordinate + Debug,
    G: DirectedGraph<EV, NV>,
{
    let progress_bar = ProgressBar::new(
        graph
            .node_count()
            .try_into()
            .expect("Value doesn't fit in u64"),
    );
    let root = graph.boundary().clone();
    let mut queue = VecDeque::new();

    let mut result = vec![];

    queue.push_back((root, root));

    while let Some(block) = queue.pop_front() {
        if block.0 != block.1 {
            let mut points = (graph.query_points(block.0), graph.query_points(block.1));
            let (Some(s), Some(t)) = (points.0.next(), points.1.next()) else {
                info!("At least one block is empty. {:?}", &block);
                continue;
            };

            let values = Values {
                d_st: graph
                    .dijkstra(s.1, HashSet::from([t.1]), Direction::Outgoing)
                    .unwrap()
                    .get(t.1)
                    .unwrap()
                    .cost()
                    .0,
                d_sp: graph
                    .dijkstra(s.1, HashSet::from([poi]), Direction::Outgoing)
                    .unwrap()
                    .get(poi)
                    .unwrap()
                    .cost()
                    .0,
                d_pt: graph
                    .dijkstra(poi, HashSet::from([t.1]), Direction::Outgoing)
                    .unwrap()
                    .get(t.1)
                    .unwrap()
                    .cost()
                    .0,
                r_af: graph.radius(s.1, &block.0, Direction::Outgoing).unwrap(),
                r_ab: graph.radius(s.1, &block.0, Direction::Incoming).unwrap(),
                r_bf: graph.radius(t.1, &block.1, Direction::Outgoing).unwrap(),
                r_bb: graph.radius(t.1, &block.1, Direction::Incoming).unwrap(),
            };

            if values.in_path(epsilon) {
                info!("Found block pair: {:#?}", &block);
                result.push(block);
                progress_bar.inc(
                    (points.0.size_hint().0 + points.1.size_hint().0 + 2)
                        .try_into()
                        .expect("value doesn't fit in u64"),
                );
            } else if values.not_in_path(epsilon) {
                progress_bar.inc(
                    (points.0.size_hint().0 + points.1.size_hint().0 + 2)
                        .try_into()
                        .expect("value doesn't fit in u64"),
                );
                continue;
            }
        }

        let children = (divide(&block.0).into_iter(), divide(&block.1).into_iter());

        let children = (
            children
                .0
                .filter(|block| graph.query(*block).peekable().peek().is_some())
                .collect::<Vec<_>>(),
            children
                .1
                .filter(|block| graph.query(*block).peekable().peek().is_some())
                .collect::<Vec<_>>(),
        );
        let mut child_block_pairs: Vec<_> = children
            .0
            .into_iter()
            .flat_map(|block_a| {
                children
                    .1
                    .iter()
                    .map(move |block_b| (block_a, block_b.clone()))
            })
            .collect();

        info!("Inserting blocks {:#?}", &child_block_pairs);

        queue.append(&mut child_block_pairs.into());
    }

    result
}

struct Values<T: FloatCore> {
    d_st: T,
    d_sp: T,
    d_pt: T,
    r_af: T,
    r_ab: T,
    r_bf: T,
    r_bb: T,
}

impl<T: FloatCore> Values<T> {
    fn in_path(&self, epsilon: T) -> bool {
        (self.r_ab + self.d_sp + self.d_pt + self.r_bf) / (self.d_st - (self.r_af + self.r_bb))
            - T::from(1).unwrap()
            <= epsilon
    }

    fn not_in_path(&self, epsilon: T) -> bool {
        (self.d_sp + self.d_pt - (self.r_ab + self.r_bf)) / (self.d_st + (self.r_ab + self.r_bf))
            - T::from(1).unwrap()
            >= epsilon
    }
}

fn divide<C: qutee::Coordinate>(block: &Boundary<C>) -> [Boundary<C>; 4] {
    let half_block_height = block.height()
        / C::from(2).unwrap_or_else(|| {
            panic!(
                "Could not convert '2' into type '{}'",
                std::any::type_name::<C>()
            )
        });
    let half_block_width = block.width()
        / C::from(2).unwrap_or_else(|| {
            panic!(
                "Could not convert '2' into type '{}'",
                std::any::type_name::<C>()
            )
        });
    let minus_one = C::from(-1).unwrap_or_else(|| {
        panic!(
            "Could not convert '-1' into type '{}'",
            std::any::type_name::<C>()
        )
    });
    let top_left = block.top_left();
    let corner_points = [
        *top_left,
        Point::new(top_left.x + half_block_width, top_left.y),
        Point::new(top_left.x, top_left.y - half_block_height),
        Point::new(
            top_left.x + half_block_width,
            top_left.y - half_block_height,
        ),
    ];

    corner_points
        .into_iter()
        .map(|corner| Boundary::new(corner, half_block_width, half_block_height * minus_one))
        .collect::<Vec<Boundary<C>>>()
        .try_into()
        .unwrap_or_else(|_| {
            panic!(
                "Could not convert 'Vec<Boundary<{}> to '[Boundary<{}>; 4]'",
                std::any::type_name::<C>(),
                std::any::type_name::<C>()
            )
        })
}

#[cfg(test)]
mod test {
    use std::ops::Bound;

    use qutee::{Boundary, Point};

    use super::divide;

    #[test]
    fn corner_points_test() {
        let block = Boundary::new(Point::new(-0.5, 0.5), 1.0, -1.0);
        dbg!(block);
        let split = divide(&block);
        let expected = [
            Boundary::new(Point::new(-0.5, 0.5), 0.5, -0.5),
            Boundary::new(Point::new(0.0, 0.5), 0.5, -0.5),
            Boundary::new(Point::new(-0.5, 0.0), 0.5, -0.5),
            Boundary::new(Point::new(0.0, 0.0), 0.5, -0.5),
        ];

        assert_eq!(split, expected);
    }
}
