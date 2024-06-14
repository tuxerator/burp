use std::{cmp::max, error::Error, iter::Copied, slice::Iter, str::FromStr, usize};

use super::Direction;

#[derive(Debug)]
pub struct EdgeList<EV> {
    edges: Box<[(usize, usize, EV)]>,
    max_node_id: usize,
}

impl<EV: Copy> EdgeList<EV> {
    pub fn new(edges: Vec<(usize, usize, EV)>) -> Self {
        let m_node_id = *edges
            .iter()
            .map(|(s, t, _)| max(s, t))
            .reduce(|acc, e| max(acc, e))
            .unwrap_or(&0);
        Self {
            edges: edges.into_boxed_slice(),
            max_node_id: m_node_id,
        }
    }

    pub fn degrees(&self, direction: Direction) -> Vec<usize> {
        let mut degrees = Vec::with_capacity(self.max_node_id + 1);
        degrees.resize_with(self.max_node_id + 1, || 0);

        if matches!(direction, Direction::Outgoing | Direction::Undirected) {
            self.edges.iter().for_each(|(s, _, _)| {
                degrees[*s] += 1;
            });
        }

        if matches!(direction, Direction::Incoming | Direction::Undirected) {
            self.edges.iter().for_each(|(_, t, _)| {
                degrees[*t] += 1;
            });
        }

        degrees
    }

    pub fn max_node_id(&self) -> usize {
        self.max_node_id
    }

    pub fn edges(&self) -> Copied<Iter<'_, (usize, usize, EV)>> {
        self.edges.iter().copied()
    }
}

impl TryFrom<&String> for EdgeList<usize>
where
    usize: FromStr,
    <usize as FromStr>::Err: std::error::Error,
{
    type Error = Box<dyn Error>;

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        let mut result: Vec<(usize, usize, usize)> = vec![];
        let mut value_iter = value.lines();

        value_iter.try_for_each(|line| -> Result<(), Box<dyn Error>> {
            let mut tokens = line.split(' ');

            let source = usize::from_str(tokens.next().ok_or("No source found!")?);

            let target = usize::from_str(tokens.next().ok_or("No target found!")?);

            let value = usize::from_str(tokens.next().ok_or("No value found!")?);

            result.push((source?, target?, value?));

            Ok(())
        })?;

        Ok(EdgeList::new(result))
    }
}

#[cfg(test)]
mod tests {
    use super::EdgeList;

    #[test]
    fn edgelist_from_string() {
        let string = "1 2 5\n\
            1 4 3\n\
            2 3 1\n\
            2 1 1\n\
            4 0 4";

        let edge_list = EdgeList::try_from(&string.to_string()).unwrap();

        assert_eq!(edge_list.max_node_id(), 4);
        assert_eq!(
            edge_list.edges().collect::<Vec<(usize, usize, usize)>>(),
            vec![(1, 2, 5), (1, 4, 3), (2, 3, 1), (2, 1, 1), (4, 0, 4)]
        );
    }

    #[test]
    #[should_panic(expected = "No value found!")]
    fn edge_list_from_string_panic() {
        let string = "1 2 5\n\
            1 4 3\n\
            2 3\n\
            2 1 1\n\
            4 0 4";

        EdgeList::try_from(&string.to_string()).unwrap();
    }
}
