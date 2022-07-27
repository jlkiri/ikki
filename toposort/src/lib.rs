use std::collections::{HashSet, VecDeque};

use multimap::MultiMap;

#[derive(Debug)]
pub struct Dag<Node: Eq + std::hash::Hash> {
    precedence: MultiMap<Node, Node>,
}

pub trait Toposort<Node> {
    fn toposort(&self) -> Option<Vec<Vec<Node>>>;
}

impl<Node> Dag<Node>
where
    Node: Eq + std::hash::Hash + Clone,
{
    pub fn new() -> Self {
        Self {
            precedence: MultiMap::new(),
        }
    }

    pub fn before(&mut self, first: Node, second: Node) {
        self.precedence.insert(first, second)
    }

    fn has_parent(&self, node: &Node) -> bool {
        self.precedence
            .iter_all()
            .any(|(_, children)| children.contains(node))
    }

    fn has_parents_except(
        &self,
        node: &Node,
        except: &Node,
        removed_edges: &HashSet<&Node>,
    ) -> bool {
        self.precedence.iter_all().any(|(parent, children)| {
            parent != except && !removed_edges.contains(&parent) && children.contains(node)
        })
    }

    fn orphans(&self) -> VecDeque<&Node> {
        let mut no_incoming_edge = VecDeque::new();
        for node in self.precedence.keys() {
            if !self.has_parent(node) {
                no_incoming_edge.push_back(node);
            }
        }
        no_incoming_edge
    }
}

impl<Node> Toposort<Node> for Dag<Node>
where
    Node: Eq + std::hash::Hash + Clone,
{
    fn toposort(&self) -> Option<Vec<Vec<Node>>> {
        let mut order = vec![];
        let mut orphans = self.orphans();
        let mut removed_edges = HashSet::new();

        while !orphans.is_empty() {
            let mut sorted = Vec::with_capacity(orphans.len());
            for _ in 0..orphans.len() {
                let node = orphans.pop_front().unwrap();
                if let Some(children) = self.precedence.get_vec(node) {
                    for child in children {
                        if !self.has_parents_except(child, node, &removed_edges) {
                            orphans.push_back(child);
                        }
                    }
                }
                removed_edges.insert(node);
                sorted.push(node.clone());
            }
            order.push(sorted)
        }

        if removed_edges.len() < self.precedence.keys().len() {
            return None;
        }

        Some(order)
    }
}

#[cfg(test)]
mod tests {
    use super::{Dag, Toposort};

    #[test]
    fn normal_sort() {
        let mut dag = Dag::new();
        dag.before("a", "b");
        dag.before("b", "c");
        let order = dag.toposort();
        let expected = vec![vec!["a"], vec!["b"], vec!["c"]];
        assert_eq!(order, Some(expected));
    }

    #[test]
    fn dag_with_cycle() {
        let mut dag = Dag::new();
        dag.before("a", "b");
        dag.before("b", "a");
        let order = dag.toposort();
        assert!(order.is_none());
    }

    #[test]
    fn parallel_when_ambiguous() {
        let mut dag = Dag::new();
        dag.before("a", "b");
        dag.before("c", "b");
        dag.before("d", "b");
        let order = dag.toposort();
        assert!(matches!(order, Some(_)));
        let mut order = order.unwrap();
        for suborder in order.iter_mut() {
            suborder.sort();
        }
        let expected = vec![vec!["a", "c", "d"], vec!["b"]];
        assert_eq!(order, expected);
    }

    #[test]
    fn parallel_when_ambiguous_2() {
        let mut dag = Dag::new();
        dag.before("a", "b");
        dag.before("a", "c");
        let order = dag.toposort();
        assert!(matches!(order, Some(_)));
        let mut order = order.unwrap();
        for suborder in order.iter_mut() {
            suborder.sort();
        }
        let expected = vec![vec!["a"], vec!["b", "c"]];
        assert_eq!(order, expected);
    }
}
