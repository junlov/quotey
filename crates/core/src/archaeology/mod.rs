use std::collections::{BTreeSet, HashMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::domain::product::{Product, ProductId};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConstraintEdgeType {
    Requires,
    Excludes,
    Alternative,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogConstraint {
    pub from: ProductId,
    pub to: ProductId,
    pub edge_type: ConstraintEdgeType,
    pub condition: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyNode {
    pub product_id: ProductId,
    pub catalog_active: bool,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyEdge {
    pub from: ProductId,
    pub to: ProductId,
    pub edge_type: ConstraintEdgeType,
    pub condition: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DependencyGraph {
    pub nodes: Vec<DependencyNode>,
    pub edges: Vec<DependencyEdge>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphBlockage {
    pub blocker: ProductId,
    pub edge_type: ConstraintEdgeType,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolutionPath {
    pub steps: Vec<ProductId>,
    pub rationale: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphAnalysis {
    pub target: ProductId,
    pub bfs_chain: Vec<ProductId>,
    pub dfs_chain: Vec<ProductId>,
    pub shortest_enablement_path: Option<Vec<ProductId>>,
    pub root_causes: Vec<ProductId>,
    pub blockages: Vec<GraphBlockage>,
    pub alternatives: Vec<ResolutionPath>,
}

#[derive(Clone, Debug, Default)]
pub struct DependencyGraphEngine;

impl DependencyGraphEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn build_graph(
        &self,
        products: &[Product],
        constraints: &[CatalogConstraint],
        selected_products: &[ProductId],
    ) -> DependencyGraph {
        let selected = selected_products
            .iter()
            .map(|product_id| product_id.0.clone())
            .collect::<BTreeSet<_>>();

        let mut nodes = products
            .iter()
            .map(|product| DependencyNode {
                product_id: product.id.clone(),
                catalog_active: product.active,
                selected: selected.contains(&product.id.0),
            })
            .collect::<Vec<_>>();
        let mut seen_nodes =
            nodes.iter().map(|node| node.product_id.0.clone()).collect::<BTreeSet<_>>();

        for constraint in constraints {
            if seen_nodes.insert(constraint.from.0.clone()) {
                nodes.push(DependencyNode {
                    product_id: constraint.from.clone(),
                    catalog_active: true,
                    selected: selected.contains(&constraint.from.0),
                });
            }
            if seen_nodes.insert(constraint.to.0.clone()) {
                nodes.push(DependencyNode {
                    product_id: constraint.to.clone(),
                    catalog_active: true,
                    selected: selected.contains(&constraint.to.0),
                });
            }
        }

        let edges = constraints
            .iter()
            .map(|constraint| DependencyEdge {
                from: constraint.from.clone(),
                to: constraint.to.clone(),
                edge_type: constraint.edge_type.clone(),
                condition: constraint.condition.clone(),
            })
            .collect::<Vec<_>>();

        DependencyGraph { nodes, edges }
    }

    pub fn bfs_dependency_chain(
        &self,
        graph: &DependencyGraph,
        start: &ProductId,
        max_depth: usize,
    ) -> Vec<ProductId> {
        let adjacency = requires_adjacency(graph);
        let mut visited = BTreeSet::new();
        let mut ordered = Vec::new();
        let mut queue = VecDeque::from([(start.clone(), 0usize)]);

        while let Some((current, depth)) = queue.pop_front() {
            if !visited.insert(current.0.clone()) {
                continue;
            }
            ordered.push(current.clone());
            if depth >= max_depth {
                continue;
            }

            for neighbor in adjacency.get(&current.0).into_iter().flatten() {
                queue.push_back((neighbor.clone(), depth + 1));
            }
        }

        ordered
    }

    pub fn dfs_dependency_chain(
        &self,
        graph: &DependencyGraph,
        start: &ProductId,
        max_depth: usize,
    ) -> Vec<ProductId> {
        let adjacency = requires_adjacency(graph);
        let mut ordered = Vec::new();
        let mut visited = BTreeSet::new();
        let mut stack = vec![(start.clone(), 0usize)];

        while let Some((current, depth)) = stack.pop() {
            if !visited.insert(current.0.clone()) {
                continue;
            }
            ordered.push(current.clone());
            if depth >= max_depth {
                continue;
            }

            let mut neighbors = adjacency.get(&current.0).cloned().unwrap_or_default();
            neighbors.reverse();
            for neighbor in neighbors {
                stack.push((neighbor, depth + 1));
            }
        }

        ordered
    }

    pub fn shortest_enablement_path(
        &self,
        graph: &DependencyGraph,
        target: &ProductId,
    ) -> Option<Vec<ProductId>> {
        let selected = selected_products(graph);
        if selected.contains(&target.0) {
            return Some(vec![target.clone()]);
        }

        let adjacency = requires_adjacency(graph);
        let mut queue = VecDeque::from([(target.clone(), vec![target.clone()])]);
        let mut visited = BTreeSet::new();
        let mut first_missing_path: Option<Vec<ProductId>> = None;

        while let Some((current, path_to_dependency)) = queue.pop_front() {
            if !visited.insert(current.0.clone()) {
                continue;
            }

            for dependency in adjacency.get(&current.0).into_iter().flatten() {
                let mut next_path = path_to_dependency.clone();
                next_path.push(dependency.clone());

                if !selected.contains(&dependency.0) {
                    let dependency_first = reverse_path(&next_path);
                    if first_missing_path.is_none() {
                        first_missing_path = Some(dependency_first.clone());
                    }

                    if direct_requires_selected(&adjacency, &selected, dependency) {
                        return Some(dependency_first);
                    }
                }

                queue.push_back((dependency.clone(), next_path));
            }
        }

        first_missing_path.or_else(|| Some(vec![target.clone()]))
    }

    pub fn identify_root_causes(
        &self,
        graph: &DependencyGraph,
        target: &ProductId,
    ) -> Vec<ProductId> {
        let selected = selected_products(graph);
        let adjacency = requires_adjacency(graph);
        let mut roots = BTreeSet::new();

        for edge in graph.edges.iter().filter(|edge| edge.from == *target) {
            match edge.edge_type {
                ConstraintEdgeType::Requires => {
                    if !selected.contains(&edge.to.0) {
                        collect_missing_leaf_dependencies(
                            &adjacency, &selected, &edge.to, &mut roots,
                        );
                    }
                }
                ConstraintEdgeType::Excludes => {
                    if selected.contains(&edge.to.0) {
                        roots.insert(edge.to.0.clone());
                    }
                }
                ConstraintEdgeType::Alternative => {}
            }
        }

        roots.into_iter().map(ProductId).collect()
    }

    pub fn analyze(
        &self,
        graph: &DependencyGraph,
        target: &ProductId,
        max_depth: usize,
    ) -> GraphAnalysis {
        let bfs_chain = self.bfs_dependency_chain(graph, target, max_depth);
        let dfs_chain = self.dfs_dependency_chain(graph, target, max_depth);
        let shortest_enablement_path = self.shortest_enablement_path(graph, target);
        let blockages = self.detect_blockages(graph, target);
        let root_causes = self.identify_root_causes(graph, target);
        let alternatives = self.alternative_paths(graph, target, shortest_enablement_path.clone());

        GraphAnalysis {
            target: target.clone(),
            bfs_chain,
            dfs_chain,
            shortest_enablement_path,
            root_causes,
            blockages,
            alternatives,
        }
    }

    fn detect_blockages(&self, graph: &DependencyGraph, target: &ProductId) -> Vec<GraphBlockage> {
        let selected = selected_products(graph);
        let mut blockages = Vec::new();

        for edge in graph.edges.iter().filter(|edge| edge.from == *target) {
            match edge.edge_type {
                ConstraintEdgeType::Requires => {
                    if !selected.contains(&edge.to.0) {
                        blockages.push(GraphBlockage {
                            blocker: edge.to.clone(),
                            edge_type: ConstraintEdgeType::Requires,
                            reason: format!(
                                "{} requires missing dependency {}",
                                edge.from.0, edge.to.0
                            ),
                        });
                    }
                }
                ConstraintEdgeType::Excludes => {
                    if selected.contains(&edge.to.0) {
                        blockages.push(GraphBlockage {
                            blocker: edge.to.clone(),
                            edge_type: ConstraintEdgeType::Excludes,
                            reason: format!(
                                "{} is blocked because {} is selected",
                                edge.from.0, edge.to.0
                            ),
                        });
                    }
                }
                ConstraintEdgeType::Alternative => {}
            }
        }

        blockages
    }

    fn alternative_paths(
        &self,
        graph: &DependencyGraph,
        target: &ProductId,
        shortest_enablement_path: Option<Vec<ProductId>>,
    ) -> Vec<ResolutionPath> {
        let mut alternatives = Vec::new();

        if let Some(path) = shortest_enablement_path {
            alternatives.push(ResolutionPath {
                steps: path,
                rationale: "Shortest dependency path to unlock target feature".to_string(),
            });
        }

        for edge in graph.edges.iter().filter(|edge| {
            edge.from == *target && edge.edge_type == ConstraintEdgeType::Alternative
        }) {
            alternatives.push(ResolutionPath {
                steps: vec![edge.to.clone()],
                rationale: format!("Use alternative {} when {} is blocked", edge.to.0, edge.from.0),
            });
        }

        alternatives
    }
}

fn requires_adjacency(graph: &DependencyGraph) -> HashMap<String, Vec<ProductId>> {
    let mut adjacency: HashMap<String, Vec<ProductId>> = HashMap::new();
    for edge in graph.edges.iter().filter(|edge| edge.edge_type == ConstraintEdgeType::Requires) {
        adjacency.entry(edge.from.0.clone()).or_default().push(edge.to.clone());
    }
    adjacency
}

fn selected_products(graph: &DependencyGraph) -> BTreeSet<String> {
    graph.nodes.iter().filter(|node| node.selected).map(|node| node.product_id.0.clone()).collect()
}

fn reverse_path(path: &[ProductId]) -> Vec<ProductId> {
    let mut reversed = path.to_vec();
    reversed.reverse();
    reversed
}

fn direct_requires_selected(
    adjacency: &HashMap<String, Vec<ProductId>>,
    selected: &BTreeSet<String>,
    dependency: &ProductId,
) -> bool {
    adjacency.get(&dependency.0).into_iter().flatten().all(|nested| selected.contains(&nested.0))
}

fn collect_missing_leaf_dependencies(
    adjacency: &HashMap<String, Vec<ProductId>>,
    selected: &BTreeSet<String>,
    node: &ProductId,
    roots: &mut BTreeSet<String>,
) {
    let requires = adjacency.get(&node.0).cloned().unwrap_or_default();
    let missing_children = requires
        .into_iter()
        .filter(|dependency| !selected.contains(&dependency.0))
        .collect::<Vec<_>>();

    if missing_children.is_empty() {
        roots.insert(node.0.clone());
        return;
    }

    for missing in missing_children {
        collect_missing_leaf_dependencies(adjacency, selected, &missing, roots);
    }
}

#[cfg(test)]
mod tests {
    use super::{CatalogConstraint, ConstraintEdgeType, DependencyGraphEngine, ResolutionPath};
    use crate::domain::product::{Product, ProductId};

    #[test]
    fn builds_graph_and_traverses_with_bfs_and_dfs() {
        let engine = DependencyGraphEngine::new();
        let graph = engine.build_graph(&products(), &constraints(), &selected_products());
        let target = ProductId("premium_analytics".to_string());

        let bfs = engine.bfs_dependency_chain(&graph, &target, 5);
        let dfs = engine.dfs_dependency_chain(&graph, &target, 5);

        assert_eq!(
            bfs,
            vec![
                ProductId("premium_analytics".to_string()),
                ProductId("api_access".to_string()),
                ProductId("enterprise_hosting".to_string()),
            ]
        );
        assert_eq!(dfs, bfs);
    }

    #[test]
    fn finds_shortest_enablement_path_and_root_causes_for_blocked_feature() {
        let engine = DependencyGraphEngine::new();
        let graph = engine.build_graph(&products(), &constraints(), &selected_products());
        let target = ProductId("premium_analytics".to_string());

        let shortest = engine.shortest_enablement_path(&graph, &target).expect("shortest path");
        let root_causes = engine.identify_root_causes(&graph, &target);

        assert_eq!(
            shortest,
            vec![
                ProductId("enterprise_hosting".to_string()),
                ProductId("api_access".to_string()),
                ProductId("premium_analytics".to_string()),
            ]
        );
        assert!(root_causes.contains(&ProductId("enterprise_hosting".to_string())));
        assert!(root_causes.contains(&ProductId("standard_hosting".to_string())));
    }

    #[test]
    fn graph_analysis_returns_paths_blockages_and_alternatives() {
        let engine = DependencyGraphEngine::new();
        let graph = engine.build_graph(&products(), &constraints(), &selected_products());
        let target = ProductId("premium_analytics".to_string());

        let analysis = engine.analyze(&graph, &target, 5);

        assert_eq!(analysis.target, target);
        assert!(!analysis.blockages.is_empty());
        assert!(analysis.shortest_enablement_path.is_some());
        assert!(analysis
            .alternatives
            .iter()
            .any(|path| matches_alternative(path, "standard_analytics")));
    }

    fn matches_alternative(path: &ResolutionPath, product_id: &str) -> bool {
        path.steps.len() == 1 && path.steps[0] == ProductId(product_id.to_string())
    }

    fn products() -> Vec<Product> {
        vec![
            product("premium_analytics"),
            product("api_access"),
            product("enterprise_hosting"),
            product("standard_hosting"),
            product("standard_analytics"),
        ]
    }

    fn selected_products() -> Vec<ProductId> {
        vec![ProductId("standard_hosting".to_string())]
    }

    fn constraints() -> Vec<CatalogConstraint> {
        vec![
            CatalogConstraint {
                from: ProductId("premium_analytics".to_string()),
                to: ProductId("api_access".to_string()),
                edge_type: ConstraintEdgeType::Requires,
                condition: Some("api_access=true".to_string()),
            },
            CatalogConstraint {
                from: ProductId("api_access".to_string()),
                to: ProductId("enterprise_hosting".to_string()),
                edge_type: ConstraintEdgeType::Requires,
                condition: Some("hosting=enterprise".to_string()),
            },
            CatalogConstraint {
                from: ProductId("premium_analytics".to_string()),
                to: ProductId("standard_hosting".to_string()),
                edge_type: ConstraintEdgeType::Excludes,
                condition: Some("incompatible_hosting".to_string()),
            },
            CatalogConstraint {
                from: ProductId("premium_analytics".to_string()),
                to: ProductId("standard_analytics".to_string()),
                edge_type: ConstraintEdgeType::Alternative,
                condition: None,
            },
        ]
    }

    fn product(product_id: &str) -> Product {
        Product::simple(product_id, format!("SKU-{product_id}"), product_id.replace('_', " "))
    }
}
