use kdl::{KdlDocument, KdlNode};
use toposort::Dag;

fn child_nodes(doc_node: &KdlNode) -> &[KdlNode] {
    doc_node
        .children()
        .map(KdlDocument::nodes)
        .unwrap_or_default()
}

fn traverse(node: &KdlNode, dag: &mut Dag<String>) {
    for child in child_nodes(node).iter() {
        let name = child.name().to_string();
        dag.before(name, node.name().to_string());
        traverse(child, dag)
    }
}

pub fn parse_deps(dependencies_node: &KdlNode) -> Dag<String> {
    let mut dag = Dag::new();
    for child in child_nodes(dependencies_node) {
        traverse(child, &mut dag);
    }
    dag
}

#[cfg(test)]
mod tests {
    use crate::{parse::parse, IkkiConfigError};

    use super::*;
    use toposort::Toposort;

    fn parse_deps_from_string(input: &str) -> Result<Dag<String>, IkkiConfigError> {
        let doc: KdlDocument = input.parse().expect("failed to parse");
        let dependencies = doc.get("dependencies").expect("no dependencies");
        Ok(parse_deps(dependencies))
    }

    #[test]
    fn correct_dependency_order() {
        let input = include_str!("../fixtures/dependencies.kdl");
        let order = parse_deps_from_string(input).unwrap();
        let order = order.toposort();

        assert!(order.is_some());

        let mut expected = vec![
            vec!["grape", "mango", "melon", "tangerine", "papaiya"],
            vec!["watermelon", "peach"],
            vec!["apple", "orange"],
            vec!["banana"],
        ];

        let mut order = order.unwrap();
        for suborder in order.iter_mut() {
            suborder.sort();
        }
        for suborder in expected.iter_mut() {
            suborder.sort();
        }

        assert_eq!(order, expected);
    }

    #[test]
    fn no_depedencies() {
        let input = include_str!("../fixtures/nodeps.kdl");
        let order = parse("nodeps.kdl", input).unwrap();

        let expected = vec![vec![
            "frontend".to_string(),
            "backend".to_string(),
            "cli".to_string(),
        ]];

        assert_eq!(order.build_order(), expected);
    }
}
