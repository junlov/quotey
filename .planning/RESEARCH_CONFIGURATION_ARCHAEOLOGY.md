# Configuration Archaeology (FEAT-09) - Technical Research

**Feature:** Constraint Graph Forensics  
**Researcher:** ResearchAgent (kimi-k2)  
**Date:** 2026-02-23  
**Priority:** P2

---

## 1. Technical Overview

Natural language forensics for constraint chains. Users ask "Why can't I add X?" and get a full dependency walkback with root cause analysis and resolution suggestions.

---

## 2. Core Components

### 2.1 Dependency Graph Builder

```rust
pub struct DependencyGraphBuilder {
    constraint_engine: Arc<dyn ConstraintEngine>,
    catalog: Arc<dyn Catalog>,
}

pub struct DependencyGraph {
    pub nodes: Vec<DependencyNode>,
    pub edges: Vec<DependencyEdge>,
}

pub struct DependencyNode {
    pub node_id: String,
    pub node_type: NodeType,
    pub name: String,
    pub status: NodeStatus,
    pub metadata: HashMap<String, String>,
}

pub enum NodeType {
    Product,
    Feature,
    Attribute,
    Constraint,
}

pub enum NodeStatus {
    Present,
    Missing,
    Blocked { reason: String },
    Optional,
}

pub struct DependencyEdge {
    pub from: String,
    pub to: String,
    pub edge_type: EdgeType,
    pub condition: Option<String>,
}

pub enum EdgeType {
    Requires,
    Excludes,
    Recommends,
    Enables,
}
```

### 2.2 Graph Construction

```rust
impl DependencyGraphBuilder {
    pub fn build(&self, quote: &Quote) -> Result<DependencyGraph, GraphError> {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        
        // Add nodes for existing line items
        for line in &quote.lines {
            let product = self.catalog.get_product(&line.product_id)?;
            
            nodes.push(DependencyNode {
                node_id: line.product_id.0.clone(),
                node_type: NodeType::Product,
                name: product.name.clone(),
                status: NodeStatus::Present,
                metadata: product.attributes.clone(),
            });
            
            // Add feature nodes for product
            for feature in &product.features {
                let feature_id = format!("{}:{}", line.product_id.0, feature.key);
                nodes.push(DependencyNode {
                    node_id: feature_id.clone(),
                    node_type: NodeType::Feature,
                    name: feature.name.clone(),
                    status: if feature.enabled {
                        NodeStatus::Present
                    } else {
                        NodeStatus::Missing
                    },
                    metadata: HashMap::new(),
                });
                
                edges.push(DependencyEdge {
                    from: line.product_id.0.clone(),
                    to: feature_id,
                    edge_type: EdgeType::Enables,
                    condition: None,
                });
            }
        }
        
        // Add constraint edges
        let constraints = self.constraint_engine.get_all_constraints()?;
        for constraint in constraints {
            match constraint.constraint_type {
                ConstraintType::Requires { source, target } => {
                    edges.push(DependencyEdge {
                        from: source.0,
                        to: target.0,
                        edge_type: EdgeType::Requires,
                        condition: constraint.condition.clone(),
                    });
                }
                ConstraintType::Excludes { source, target } => {
                    edges.push(DependencyEdge {
                        from: source.0,
                        to: target.0,
                        edge_type: EdgeType::Excludes,
                        condition: None,
                    });
                }
                _ => {}
            }
        }
        
        Ok(DependencyGraph { nodes, edges })
    }
    
    pub fn add_hypothetical(
        &self,
        graph: &mut DependencyGraph,
        product_id: &ProductId,
    ) -> Result<(), GraphError> {
        let product = self.catalog.get_product(product_id)?;
        
        graph.nodes.push(DependencyNode {
            node_id: product_id.0.clone(),
            node_type: NodeType::Product,
            name: product.name.clone(),
            status: NodeStatus::Missing,  // Hypothetical
            metadata: product.attributes.clone(),
        });
        
        Ok(())
    }
}
```

---

## 3. Blockage Detection

```rust
pub struct BlockageAnalyzer;

impl BlockageAnalyzer {
    /// Find why a product cannot be added
    pub fn analyze_blockage(
        &self,
        graph: &DependencyGraph,
        target_product: &ProductId,
    ) -> Option<BlockageReport> {
        let target_node = graph.nodes.iter()
            .find(|n| n.node_id == target_product.0)?;
        
        let mut blockages = Vec::new();
        
        // Check for unsatisfied requirements
        let required_edges: Vec<_> = graph.edges.iter()
            .filter(|e| e.to == target_product.0 && e.edge_type == EdgeType::Requires)
            .collect();
        
        for edge in required_edges {
            let source_node = graph.nodes.iter()
                .find(|n| n.node_id == edge.from);
            
            match source_node {
                None => {
                    // Required product not in quote
                    blockages.push(Blockage {
                        blockage_type: BlockageType::MissingDependency,
                        description: format!("Requires {} (not in quote)", edge.from),
                        node_id: edge.from.clone(),
                        resolution_hint: format!("Add {} first", edge.from),
                    });
                }
                Some(node) if matches!(node.status, NodeStatus::Present) => {
                    // Required product present, check if it has blockages
                    if let Some(nested) = self.analyze_blockage(graph, &ProductId(edge.from.clone())) {
                        blockages.push(Blockage {
                            blockage_type: BlockageType::NestedDependency,
                            description: format!("Requires {} which has blockages", edge.from),
                            node_id: edge.from.clone(),
                            resolution_hint: nested.summary(),
                        });
                    }
                }
                _ => {}
            }
        }
        
        // Check for exclusions
        let excluded_edges: Vec<_> = graph.edges.iter()
            .filter(|e| e.to == target_product.0 && e.edge_type == EdgeType::Excludes)
            .collect();
        
        for edge in excluded_edges {
            let source_node = graph.nodes.iter()
                .find(|n| n.node_id == edge.from);
            
            if let Some(node) = source_node {
                if matches!(node.status, NodeStatus::Present) {
                    blockages.push(Blockage {
                        blockage_type: BlockageType::ExclusionConflict,
                        description: format!("Incompatible with {} (already in quote)", edge.from),
                        node_id: edge.from.clone(),
                        resolution_hint: format!("Remove {} to add {}", edge.from, target_product.0),
                    });
                }
            }
        }
        
        if blockages.is_empty() {
            None
        } else {
            Some(BlockageReport {
                target_product: target_product.0.clone(),
                blockages,
            })
        }
    }
    
    /// Find all requirements for a product
    pub fn find_requirements(
        &self,
        graph: &DependencyGraph,
        product_id: &ProductId,
    ) -> Vec<RequirementPath> {
        let mut paths = Vec::new();
        let mut visited = HashSet::new();
        
        self.dfs_requirements(
            graph,
            &product_id.0,
            &mut vec![],
            &mut visited,
            &mut paths,
        );
        
        paths
    }
    
    fn dfs_requirements(
        &self,
        graph: &DependencyGraph,
        node_id: &str,
        current_path: &mut Vec<String>,
        visited: &mut HashSet<String>,
        paths: &mut Vec<RequirementPath>,
    ) {
        if !visited.insert(node_id.to_string()) {
            return;
        }
        
        current_path.push(node_id.to_string());
        
        // Find all nodes that require this one
        let requiring: Vec<_> = graph.edges.iter()
            .filter(|e| e.to == node_id && e.edge_type == EdgeType::Requires)
            .map(|e| e.from.clone())
            .collect();
        
        if requiring.is_empty() {
            // Leaf node - save path
            paths.push(RequirementPath {
                path: current_path.clone(),
            });
        } else {
            for req in requiring {
                self.dfs_requirements(graph, &req, current_path, visited, paths);
            }
        }
        
        current_path.pop();
    }
}

pub struct BlockageReport {
    pub target_product: String,
    pub blockages: Vec<Blockage>,
}

impl BlockageReport {
    pub fn summary(&self) -> String {
        format!(
            "{} blockages for {}: {}",
            self.blockages.len(),
            self.target_product,
            self.blockages.iter()
                .map(|b| b.description.clone())
                .collect::<Vec<_>>()
                .join("; ")
        )
    }
}

pub struct Blockage {
    pub blockage_type: BlockageType,
    pub description: String,
    pub node_id: String,
    pub resolution_hint: String,
}

pub enum BlockageType {
    MissingDependency,
    NestedDependency,
    ExclusionConflict,
    AttributeMismatch,
}
```

---

## 4. Resolution Pathfinder

```rust
pub struct ResolutionPathfinder;

impl ResolutionPathfinder {
    /// Find alternative paths to enable a product
    pub fn find_resolution_paths(
        &self,
        graph: &DependencyGraph,
        blockage: &BlockageReport,
    ) -> Vec<ResolutionPath> {
        let mut paths = Vec::new();
        
        for blockage in &blockage.blockages {
            match blockage.blockage_type {
                BlockageType::MissingDependency => {
                    // Path 1: Add the missing dependency
                    paths.push(ResolutionPath {
                        steps: vec![ResolutionStep {
                            action: ResolutionAction::AddProduct {
                                product_id: blockage.node_id.clone(),
                            },
                            description: format!("Add required product: {}", blockage.node_id),
                            estimated_cost: None,
                        }],
                        total_effort: EffortLevel::Low,
                    });
                }
                BlockageType::ExclusionConflict => {
                    // Path 1: Remove the conflicting product
                    paths.push(ResolutionPath {
                        steps: vec![ResolutionStep {
                            action: ResolutionAction::RemoveProduct {
                                product_id: blockage.node_id.clone(),
                            },
                            description: format!("Remove conflicting product: {}", blockage.node_id),
                            estimated_cost: None,
                        }],
                        total_effort: EffortLevel::Low,
                    });
                    
                    // Path 2: Find alternative to target product
                    if let Some(alternative) = self.find_alternative(graph, &blockage_report.target_product) {
                        paths.push(ResolutionPath {
                            steps: vec![ResolutionStep {
                                action: ResolutionAction::UseAlternative {
                                    product_id: alternative,
                                },
                                description: "Use compatible alternative".to_string(),
                                estimated_cost: None,
                            }],
                            total_effort: EffortLevel::Low,
                        });
                    }
                }
                _ => {}
            }
        }
        
        // Rank by effort
        paths.sort_by_key(|p| p.total_effort);
        
        paths
    }
    
    fn find_alternative(&self, graph: &DependencyGraph, product_id: &str) -> Option<String> {
        // Find products with similar features but no conflicts
        graph.nodes.iter()
            .filter(|n| n.node_type == NodeType::Product && n.node_id != product_id)
            .filter(|n| matches!(n.status, NodeStatus::Present | NodeStatus::Optional))
            .map(|n| n.node_id.clone())
            .next()
    }
}

pub struct ResolutionPath {
    pub steps: Vec<ResolutionStep>,
    pub total_effort: EffortLevel,
}

pub struct ResolutionStep {
    pub action: ResolutionAction,
    pub description: String,
    pub estimated_cost: Option<Decimal>,
}

pub enum ResolutionAction {
    AddProduct { product_id: String },
    RemoveProduct { product_id: String },
    UpgradeProduct { from: String, to: String },
    UseAlternative { product_id: String },
}

pub enum EffortLevel {
    Low,
    Medium,
    High,
}
```

---

## 5. Natural Language Explanation

```rust
pub struct ExplanationGenerator;

impl ExplanationGenerator {
    pub fn generate_explanation(
        &self,
        query: &str,
        blockage: &BlockageReport,
        resolutions: &[ResolutionPath],
    ) -> String {
        let mut explanation = format!(
            "🔍 *Configuration Analysis for {}*\n\n",
            blockage.target_product
        );
        
        // Explain blockages
        explanation.push_str("*Blockages found:*\n");
        for (i, blockage) in blockage.blockages.iter().enumerate() {
            explanation.push_str(&format!(
                "{}. {}\n   → {}\n",
                i + 1,
                blockage.description,
                blockage.resolution_hint
            ));
        }
        
        // Explain resolution paths
        if !resolutions.is_empty() {
            explanation.push_str("\n*Resolution paths:*\n");
            for (i, path) in resolutions.iter().take(3).enumerate() {
                explanation.push_str(&format!(
                    "{}. {}\n",
                    i + 1,
                    path.steps.iter()
                        .map(|s| s.description.clone())
                        .collect::<Vec<_>>()
                        .join(" → ")
                ));
            }
        }
        
        explanation
    }
    
    pub fn render_tree_visualization(
        &self,
        graph: &DependencyGraph,
        root: &str,
    ) -> String {
        let mut output = String::new();
        self.render_node_recursive(graph, root, &mut output, 0, &mut HashSet::new());
        output
    }
    
    fn render_node_recursive(
        &self,
        graph: &DependencyGraph,
        node_id: &str,
        output: &mut String,
        depth: usize,
        visited: &mut HashSet<String>,
    ) {
        if !visited.insert(node_id.to_string()) {
            return;  // Cycle detected
        }
        
        let node = match graph.nodes.iter().find(|n| n.node_id == node_id) {
            Some(n) => n,
            None => return,
        };
        
        let indent = "  ".repeat(depth);
        let status_emoji = match node.status {
            NodeStatus::Present => "✓",
            NodeStatus::Missing => "❌",
            NodeStatus::Blocked { .. } => "🚫",
            NodeStatus::Optional => "○",
        };
        
        output.push_str(&format!(
            "{}{} {}\n",
            indent,
            status_emoji,
            node.name
        ));
        
        // Find children
        let children: Vec<_> = graph.edges.iter()
            .filter(|e| e.from == node_id && e.edge_type == EdgeType::Requires)
            .map(|e| e.to.clone())
            .collect();
        
        for child in children {
            self.render_node_recursive(graph, &child, output, depth + 1, visited);
        }
    }
}
```

---

## 6. Slack Integration

```rust
pub fn render_archaeology_result(
    query: &str,
    blockage: &BlockageReport,
    resolutions: &[ResolutionPath],
) -> MessageTemplate {
    let mut builder = MessageBuilder::new("Configuration Archaeology");
    
    // Header
    builder = builder.section("header", |s| {
        s.mrkdwn(format!(
            "🔍 *Why can't I add {}?*",
            blockage.target_product
        ))
    });
    
    // Dependency tree
    let tree = ExplanationGenerator.render_tree_visualization(graph, &blockage.target_product);
    builder = builder.section("tree", |s| {
        s.mrkdwn(format!("```{}```", tree))
    });
    
    // Resolution paths
    if !resolutions.is_empty() {
        builder = builder.section("resolutions", |s| {
            let paths: Vec<String> = resolutions.iter().take(3).enumerate()
                .map(|(i, path)| format!(
                    "{}. {} (effort: {:?})",
                    i + 1,
                    path.steps[0].description,
                    path.total_effort
                ))
                .collect();
            s.mrkdwn(format!("*Suggested fixes:*\n{}", paths.join("\n")))
        });
        
        // Action buttons
        builder = builder.actions("actions", |a| {
            for (i, path) in resolutions.iter().take(3).enumerate() {
                a.button(ButtonElement::new(
                    &format!("apply_{}", i),
                    &format!("Fix {}", i + 1)
                ));
            }
            a
        });
    }
    
    builder.build()
}
```

---

*Research compiled by ResearchAgent for the quotey project.*
