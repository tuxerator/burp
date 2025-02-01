use std::{
    arch::x86_64,
    cell::RefCell,
    collections::{BTreeMap, HashSet, VecDeque},
    fmt::Debug,
    iter::Peekable,
    rc::{Rc, Weak},
    usize,
};

use geo::{point, Coord, CoordFloat, CoordNum, MultiPolygon, Polygon, Rect};
use graph_rs::{
    algorithms::dijkstra::{CachedDijkstra, Dijkstra},
    graph::{quad_tree::QuadGraph, rstar::RTreeGraph},
    types::Direction,
    CoordGraph, Coordinate, DirectedGraph, Graph,
};
use indicatif::ProgressBar;
use log::{debug, info, warn};
use num_traits::Num;
use ordered_float::FloatCore;
use qutee::{Boundary, DynCap, Point, QueryPoints};
use rstar::{
    primitives::{GeomWithData, Rectangle},
    RTree, RTreeNum, RTreeObject, RTreeParams, AABB,
};

macro_rules! unwrap_or_continue {
    ($e:expr) => {
        match $e {
            Some(x) => x,
            None => continue,
        }
    };
}

struct BlockData<C>
where
    C: RTreeNum + CoordFloat,
{
    linked_block: Option<Weak<RefCell<GeomWithData<Rectangle<Coord<C>>, BlockData<C>>>>>,
    poi_id: usize,
}

impl<C> BlockData<C>
where
    C: RTreeNum + CoordFloat,
{
    fn new_unlinked(poi_id: usize) -> Self {
        BlockData {
            linked_block: None,
            poi_id,
        }
    }

    fn new_linked(
        link_block: *const GeomWithData<Rectangle<Coord<C>>, BlockData<C>>,
        poi_id: usize,
    ) -> Self {
        BlockData {
            linked_block: Some(link_block),
            poi_id,
        }
    }

    fn link(&mut self, link_block: *const GeomWithData<Rectangle<Coord<C>>, BlockData<C>>) {
        self.linked_block = Some(link_block);
    }

    fn get_linked_block(&self) -> Option<*const GeomWithData<Rectangle<Coord<C>>, BlockData<C>>> {
        self.linked_block
    }
}

pub struct Oracle<C>
where
    C: RTreeNum + CoordFloat,
{
    r_tree: RTree<GeomWithData<Rectangle<Coord<C>>, BlockData<C>>>,
}

impl<C> Oracle<C>
where
    C: RTreeNum + CoordFloat,
{
    pub fn new() -> Self {
        Oracle {
            r_tree: RTree::new(),
        }
    }

    fn add_block_pair(&mut self, s_block: Rect<C>, t_block: Rect<C>, poi: usize) {
        let s_block_data = BlockData::new_unlinked(poi);
        let mut s_geom = GeomWithData::new(
            Rectangle::from_corners(s_block.min(), s_block.max()),
            s_block_data,
        );

        let t_block_data = BlockData::new_linked(
            &s_geom as *const GeomWithData<Rectangle<Coord<C>>, BlockData<C>>,
            poi,
        );
        let mut t_geom = GeomWithData::new(
            Rectangle::from_corners(t_block.min(), t_block.max()),
            t_block_data,
        );

        s_geom
            .data
            .link(&t_geom as *const GeomWithData<Rectangle<Coord<C>>, BlockData<C>>);

        self.r_tree.insert(s_geom);
        self.r_tree.insert(t_geom);
    }

    pub fn build<EV, NV, G>(
        graph: &mut RTreeGraph<EV, NV, G, C>,
        poi: usize,
        epsilon: EV,
    ) -> Result<Self, String>
    where
        EV: FloatCore,
        NV: Coordinate<C> + Debug,
        G: DirectedGraph<EV, NV> + CachedDijkstra<EV, NV>,
    {
        // let progress_bar = ProgressBar::new( graph .node_count()
        //         .try_into()
        //         .expect("Value doesn't fit in u64"),
        // );

        let mut oracle = Oracle::<C>::new();

        let Some(root) = graph.bounding_rect() else {
            return Err("Could not get bounding rect of graph".to_string());
        };
        let mut queue = VecDeque::new();

        queue.push_back((root, root));

        while let Some(block_pair) = queue.pop_front() {
            if block_pair.0 != block_pair.1 {
                let s: GeomWithData<Coord<C>, usize>;
                let t: GeomWithData<Coord<C>, usize>;
                {
                    let mut points = (
                        graph
                            .query(&AABB::from_corners(block_pair.0.min(), block_pair.0.max()))
                            .cloned(),
                        graph
                            .query(&AABB::from_corners(block_pair.1.min(), block_pair.1.max()))
                            .cloned(),
                    );
                    let (Some(p_0), Some(p_1)) = (points.0.next(), points.1.next()) else {
                        debug!("At least one block is empty. {:?}", &block_pair);
                        continue;
                    };

                    s = p_0;
                    t = p_1;
                }

                let values = Values {
                    d_st: *unwrap_or_continue!(graph
                        .cached_dijkstra(s.data, HashSet::from([t.data]), Direction::Outgoing)
                        .unwrap()
                        .get(t.data))
                    .cost(),
                    d_sp: *unwrap_or_continue!(graph
                        .cached_dijkstra(s.data, HashSet::from([poi]), Direction::Outgoing)
                        .unwrap()
                        .get(poi))
                    .cost(),
                    d_pt: *unwrap_or_continue!(graph
                        .cached_dijkstra(poi, HashSet::from([t.data]), Direction::Outgoing)
                        .unwrap()
                        .get(t.data))
                    .cost(),
                    r_af: graph
                        .radius(
                            s.data,
                            &AABB::from_corners(block_pair.0.min(), block_pair.0.max()),
                            Direction::Outgoing,
                        )
                        .unwrap(),
                    r_ab: graph
                        .radius(
                            s.data,
                            &AABB::from_corners(block_pair.0.min(), block_pair.0.max()),
                            Direction::Incoming,
                        )
                        .unwrap(),
                    r_bf: graph
                        .radius(
                            t.data,
                            &AABB::from_corners(block_pair.1.min(), block_pair.1.max()),
                            Direction::Outgoing,
                        )
                        .unwrap(),
                    r_bb: graph
                        .radius(
                            t.data,
                            &AABB::from_corners(block_pair.1.min(), block_pair.1.max()),
                            Direction::Incoming,
                        )
                        .unwrap(),
                };

                if values.in_path(epsilon) {
                    debug!("Found in-path block pair: {:#?}", &block_pair);
                    oracle.add_block_pair(block_pair.0, block_pair.1, poi);
                    // progress_bar.inc(
                    //     (points.0.size_hint().0 + points.1.size_hint().0 + 2)
                    //         .try_into()
                    //         .expect("value doesn't fit in u64"),
                    // );

                    continue;
                } else if values.not_in_path(epsilon) {
                    debug!("Found not in-path block pair: {:#?}", &block_pair);
                    // progress_bar.inc(
                    //     (points.0.size_hint().0 + points.1.size_hint().0 + 2)
                    //         .try_into()
                    //         .expect("value doesn't fit in u64"),
                    // );
                    continue;
                }
            }

            let children = (
                block_pair
                    .0
                    .split_y()
                    .into_iter()
                    .flat_map(|split| split.split_x()),
                block_pair
                    .1
                    .split_y()
                    .into_iter()
                    .flat_map(|split| split.split_x()),
            );

            let children = (
                children
                    .0
                    .filter(|block| {
                        graph
                            .query(&AABB::from_corners(block.min(), block.max()))
                            .peekable()
                            .peek()
                            .is_some()
                    })
                    .collect::<Vec<_>>(),
                children
                    .1
                    .filter(|block| {
                        graph
                            .query(&AABB::from_corners(block.min(), block.max()))
                            .peekable()
                            .peek()
                            .is_some()
                    })
                    .collect::<Vec<_>>(),
            );
            let child_block_pairs: Vec<_> = children
                .0
                .into_iter()
                .flat_map(|block_a| children.1.iter().map(move |block_b| (block_a, *block_b)))
                .filter(|block_pair| {
                    let points = (
                        graph
                            .query(&AABB::from_corners(block_pair.0.min(), block_pair.0.max()))
                            .collect::<Vec<_>>(),
                        graph
                            .query(&AABB::from_corners(block_pair.1.min(), block_pair.1.max()))
                            .collect::<Vec<_>>(),
                    );

                    // If both blocks only contain the same node they can be discarded
                    if points.0.len() == 1 && points.0 == points.1 {
                        return false;
                    }
                    true
                })
                .collect();

            debug!("inserting {} block pairs", child_block_pairs.len());

            queue.append(&mut child_block_pairs.into());
        }

        Ok(oracle)
    }

    pub fn get_block_pairs(
        &self,
        coord: Coord<C>,
    ) -> Vec<(&Rectangle<Coord<C>>, &Rectangle<Coord<C>>)> {
        let blocks = self.r_tree.locate_all_at_point(&coord);
        let mut block_pairs = vec![];

        for block in blocks {
            // Safety: As long as r_tree lives the pointers are valid.
            unsafe {
                let other = &*block.data.linked_block.unwrap();

                block_pairs.push((block.geom(), other.geom()));
            }
        }

        block_pairs
    }
}

impl<C> Default for Oracle<C>
where
    C: RTreeNum + CoordFloat,
{
    fn default() -> Self {
        Self::new()
    }
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
        (self.r_ab + self.d_sp + self.d_pt + self.r_bf)
            <= (self.d_st - (self.r_af + self.r_bb)) * (T::from(1).unwrap() + epsilon)
    }

    fn not_in_path(&self, epsilon: T) -> bool {
        (self.d_sp + self.d_pt - (self.r_ab + self.r_bf))
            >= (self.d_st + (self.r_ab + self.r_bf)) * (T::from(1).unwrap() + epsilon)
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
    use std::{f64, ops::Bound};

    use qutee::{Area, Boundary, Point};
    use rand::random;

    use super::divide;

    #[test]
    fn corner_points_test() {
        for _ in 0..1000 {
            let point: Point<f64> = Point::new(
                random::<f64>() * 360.0 - 180.0,
                random::<f64>() * 180.0 - 90.0,
            );
            let block = Boundary::new(point, random::<f64>() * 360.0, -random::<f64>() * 180.0);
            let split = divide(&block);
            let expected = [
                Boundary::new(point, block.width() / 2.0, -block.height() / 2.0),
                Boundary::new(
                    Point::new(point.x + block.width() / 2.0, point.y),
                    block.width() / 2.0,
                    -block.height() / 2.0,
                ),
                Boundary::new(
                    Point::new(point.x, point.y - block.height() / 2.0),
                    block.width() / 2.0,
                    -block.height() / 2.0,
                ),
                Boundary::new(
                    Point::new(
                        point.x + block.width() / 2.0,
                        point.y - block.height() / 2.0,
                    ),
                    block.width() / 2.0,
                    -block.height() / 2.0,
                ),
            ];

            assert_eq!(split, expected);
            for i in 0..4 {
                for j in 0..4 {
                    if i != j {
                        assert!(!split[i].intersects(&split[j]));
                    }
                }
            }
        }
    }
}
