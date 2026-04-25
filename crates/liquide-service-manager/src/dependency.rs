// Service dependency resolution: topological ordering and cycle detection.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::service::ServiceId;

/// Tracks dependency relationships between services and provides
/// topological ordering for start/stop sequences.
#[derive(Debug)]
pub struct DependencyGraph {
    /// For each service, the set of services it depends on (must start first).
    deps: HashMap<ServiceId, HashSet<ServiceId>>,
}

impl DependencyGraph {
    /// Create an empty dependency graph.
    pub fn new() -> Self {
        Self {
            deps: HashMap::new(),
        }
    }

    /// Register a service in the graph (with no dependencies initially).
    pub fn add_service(&mut self, id: ServiceId) {
        self.deps.entry(id).or_default();
    }

    /// Remove a service and all edges referencing it.
    pub fn remove_service(&mut self, id: &ServiceId) {
        self.deps.remove(id);
        // Remove id from all dependency sets
        for dep_set in self.deps.values_mut() {
            dep_set.remove(id);
        }
    }

    /// Declare that `service` depends on `dependency` (dependency must start first).
    pub fn add_dependency(&mut self, service: ServiceId, dependency: ServiceId) {
        self.deps
            .entry(service)
            .or_default()
            .insert(dependency.clone());
        // Ensure the dependency node exists too
        self.deps.entry(dependency).or_default();
    }

    /// Remove a specific dependency edge.
    pub fn remove_dependency(&mut self, service: &ServiceId, dependency: &ServiceId) {
        if let Some(set) = self.deps.get_mut(service) {
            set.remove(dependency);
        }
    }

    /// Return the services that `id` directly depends on.
    pub fn dependencies(&self, id: &ServiceId) -> Vec<ServiceId> {
        self.deps
            .get(id)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Return the services that directly depend on `id`.
    pub fn dependents(&self, id: &ServiceId) -> Vec<ServiceId> {
        let mut result = Vec::new();
        for (svc, dep_set) in &self.deps {
            if dep_set.contains(id) {
                result.push(svc.clone());
            }
        }
        result
    }

    /// Compute a topological ordering for starting the target service,
    /// including all transitive dependencies. Returns services in
    /// start order (dependencies first, target last).
    ///
    /// Returns `None` if a cycle is detected in the relevant subgraph.
    pub fn start_order(&self, target: &ServiceId) -> Option<Vec<ServiceId>> {
        // BFS to collect all transitive deps, then topological sort
        let mut relevant = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(target.clone());

        while let Some(current) = queue.pop_front() {
            if !relevant.insert(current.clone()) {
                continue;
            }
            if let Some(dep_set) = self.deps.get(&current) {
                for dep in dep_set {
                    queue.push_back(dep.clone());
                }
            }
        }

        // Kahn's algorithm on the relevant subgraph
        self.topo_sort_subset(&relevant)
    }

    /// Compute the order in which services should be stopped when stopping `target`.
    /// This includes all transitive dependents. Returns in stop order
    /// (dependents first, target last — reverse of start order for the
    /// dependent subgraph).
    pub fn stop_order(&self, target: &ServiceId) -> Vec<ServiceId> {
        // Collect all transitive dependents
        let mut relevant = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(target.clone());

        while let Some(current) = queue.pop_front() {
            if !relevant.insert(current.clone()) {
                continue;
            }
            for dependent in self.dependents(&current) {
                queue.push_back(dependent);
            }
        }

        // Topological sort of this subgraph, then reverse
        if let Some(mut order) = self.topo_sort_subset(&relevant) {
            order.reverse();
            order
        } else {
            // If there's a cycle, return just the target
            vec![target.clone()]
        }
    }

    /// Detect if the graph contains any cycle. Returns `Some(cycle_path)`
    /// with a vec of service IDs forming the cycle, or `None` if acyclic.
    pub fn has_cycle(&self) -> Option<Vec<ServiceId>> {
        // DFS-based cycle detection with path tracking
        let mut visited = HashSet::new();
        let mut on_stack = HashSet::new();
        let mut path = Vec::new();

        for start in self.deps.keys() {
            if !visited.contains(start) {
                if let Some(cycle) = self.dfs_cycle(start, &mut visited, &mut on_stack, &mut path) {
                    return Some(cycle);
                }
            }
        }
        None
    }

    /// Check if all transitive dependencies of `target` are present in the graph.
    pub fn has_missing_dependencies(&self, target: &ServiceId) -> Vec<ServiceId> {
        let mut missing = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(target.clone());

        while let Some(current) = queue.pop_front() {
            if !visited.insert(current.clone()) {
                continue;
            }
            if let Some(dep_set) = self.deps.get(&current) {
                for dep in dep_set {
                    if !self.deps.contains_key(dep) {
                        missing.push(dep.clone());
                    } else {
                        queue.push_back(dep.clone());
                    }
                }
            }
        }
        missing
    }

    /// Total number of services in the graph.
    pub fn len(&self) -> usize {
        self.deps.len()
    }

    /// Whether the graph has no services.
    pub fn is_empty(&self) -> bool {
        self.deps.is_empty()
    }

    // --- internal helpers ---

    fn topo_sort_subset(&self, subset: &HashSet<ServiceId>) -> Option<Vec<ServiceId>> {
        // Build in-degree map for the subset
        let mut in_degree: HashMap<ServiceId, usize> = HashMap::new();
        for id in subset {
            in_degree.entry(id.clone()).or_insert(0);
            if let Some(dep_set) = self.deps.get(id) {
                for dep in dep_set {
                    if subset.contains(dep) {
                        // dep -> id edge: id's in-degree increases
                        *in_degree.entry(id.clone()).or_insert(0) += 1;
                    }
                }
            }
        }

        // BUT we also need to ensure the in-degree counting is correct.
        // Let me recount: in_degree[x] = number of edges pointing TO x within subset.
        // An edge exists from dep to service (dep must start before service).
        let mut in_deg: HashMap<ServiceId, usize> = HashMap::new();
        for id in subset {
            in_deg.entry(id.clone()).or_insert(0);
        }
        for id in subset {
            if let Some(dep_set) = self.deps.get(id) {
                for dep in dep_set {
                    if subset.contains(dep) {
                        *in_deg.entry(id.clone()).or_insert(0) += 1;
                    }
                }
            }
        }

        let mut queue: VecDeque<ServiceId> = VecDeque::new();
        for (id, &deg) in &in_deg {
            if deg == 0 {
                queue.push_back(id.clone());
            }
        }

        // Sort initial queue for deterministic output
        let mut initial: Vec<ServiceId> = queue.drain(..).collect();
        initial.sort_by(|a, b| a.0.cmp(&b.0));
        for id in initial {
            queue.push_back(id);
        }

        let mut result = Vec::new();
        while let Some(current) = queue.pop_front() {
            result.push(current.clone());
            // Find services in subset that depend on `current`
            let mut next_ready = Vec::new();
            for id in subset {
                if let Some(dep_set) = self.deps.get(id) {
                    if dep_set.contains(&current) {
                        if let Some(deg) = in_deg.get_mut(id) {
                            *deg -= 1;
                            if *deg == 0 {
                                next_ready.push(id.clone());
                            }
                        }
                    }
                }
            }
            next_ready.sort_by(|a, b| a.0.cmp(&b.0));
            for id in next_ready {
                queue.push_back(id);
            }
        }

        if result.len() == subset.len() {
            Some(result)
        } else {
            None // cycle detected
        }
    }

    fn dfs_cycle(
        &self,
        node: &ServiceId,
        visited: &mut HashSet<ServiceId>,
        on_stack: &mut HashSet<ServiceId>,
        path: &mut Vec<ServiceId>,
    ) -> Option<Vec<ServiceId>> {
        visited.insert(node.clone());
        on_stack.insert(node.clone());
        path.push(node.clone());

        if let Some(dep_set) = self.deps.get(node) {
            for dep in dep_set {
                if !visited.contains(dep) {
                    if let Some(cycle) = self.dfs_cycle(dep, visited, on_stack, path) {
                        return Some(cycle);
                    }
                } else if on_stack.contains(dep) {
                    // Found a cycle — extract the cycle from path
                    let start = path.iter().position(|x| x == dep)?;
                    let mut cycle: Vec<ServiceId> = path[start..].to_vec();
                    cycle.push(dep.clone()); // close the cycle
                    return Some(cycle);
                }
            }
        }

        on_stack.remove(node);
        path.pop();
        None
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> ServiceId {
        ServiceId::new(s)
    }

    #[test]
    fn empty_graph() {
        let g = DependencyGraph::new();
        assert!(g.is_empty());
        assert_eq!(g.len(), 0);
        assert!(g.has_cycle().is_none());
    }

    #[test]
    fn add_and_remove_service() {
        let mut g = DependencyGraph::new();
        g.add_service(id("a"));
        assert_eq!(g.len(), 1);
        g.remove_service(&id("a"));
        assert!(g.is_empty());
    }

    #[test]
    fn simple_dependency() {
        let mut g = DependencyGraph::new();
        g.add_dependency(id("app"), id("dbus"));

        let deps = g.dependencies(&id("app"));
        assert_eq!(deps, vec![id("dbus")]);

        let dependents = g.dependents(&id("dbus"));
        assert_eq!(dependents, vec![id("app")]);
    }

    #[test]
    fn remove_dependency() {
        let mut g = DependencyGraph::new();
        g.add_dependency(id("app"), id("dbus"));
        g.remove_dependency(&id("app"), &id("dbus"));
        assert!(g.dependencies(&id("app")).is_empty());
    }

    #[test]
    fn start_order_simple_chain() {
        // c depends on b depends on a
        let mut g = DependencyGraph::new();
        g.add_dependency(id("c"), id("b"));
        g.add_dependency(id("b"), id("a"));

        let order = g.start_order(&id("c")).unwrap();
        assert_eq!(order, vec![id("a"), id("b"), id("c")]);
    }

    #[test]
    fn start_order_no_deps() {
        let mut g = DependencyGraph::new();
        g.add_service(id("standalone"));
        let order = g.start_order(&id("standalone")).unwrap();
        assert_eq!(order, vec![id("standalone")]);
    }

    #[test]
    fn start_order_diamond() {
        //   d depends on b, c
        //   b depends on a
        //   c depends on a
        let mut g = DependencyGraph::new();
        g.add_dependency(id("d"), id("b"));
        g.add_dependency(id("d"), id("c"));
        g.add_dependency(id("b"), id("a"));
        g.add_dependency(id("c"), id("a"));

        let order = g.start_order(&id("d")).unwrap();
        // a must come first, then b and c (sorted alphabetically), then d
        assert_eq!(order[0], id("a"));
        assert_eq!(*order.last().unwrap(), id("d"));
        // b and c in middle
        let middle: Vec<&ServiceId> = order[1..3].iter().collect();
        assert!(middle.contains(&&id("b")));
        assert!(middle.contains(&&id("c")));
    }

    #[test]
    fn stop_order_chain() {
        // c depends on b depends on a
        let mut g = DependencyGraph::new();
        g.add_dependency(id("c"), id("b"));
        g.add_dependency(id("b"), id("a"));

        // Stopping a: must stop c first (depends on b which depends on a), then b, then a
        let order = g.stop_order(&id("a"));
        assert_eq!(order[0], id("c"));
        assert_eq!(order[1], id("b"));
        assert_eq!(order[2], id("a"));
    }

    #[test]
    fn stop_order_leaf() {
        let mut g = DependencyGraph::new();
        g.add_dependency(id("b"), id("a"));
        // Stopping b: nothing depends on b, so just [b]
        let order = g.stop_order(&id("b"));
        assert_eq!(order, vec![id("b")]);
    }

    #[test]
    fn cycle_detection_simple() {
        let mut g = DependencyGraph::new();
        g.add_dependency(id("a"), id("b"));
        g.add_dependency(id("b"), id("a"));

        let cycle = g.has_cycle();
        assert!(cycle.is_some());
        let cycle = cycle.unwrap();
        assert!(cycle.len() >= 2);
    }

    #[test]
    fn cycle_detection_three_node() {
        let mut g = DependencyGraph::new();
        g.add_dependency(id("a"), id("b"));
        g.add_dependency(id("b"), id("c"));
        g.add_dependency(id("c"), id("a"));

        assert!(g.has_cycle().is_some());
    }

    #[test]
    fn no_cycle_in_dag() {
        let mut g = DependencyGraph::new();
        g.add_dependency(id("c"), id("b"));
        g.add_dependency(id("b"), id("a"));
        g.add_dependency(id("c"), id("a"));
        assert!(g.has_cycle().is_none());
    }

    #[test]
    fn start_order_returns_none_on_cycle() {
        let mut g = DependencyGraph::new();
        g.add_dependency(id("a"), id("b"));
        g.add_dependency(id("b"), id("a"));
        assert!(g.start_order(&id("a")).is_none());
    }

    #[test]
    fn dependencies_of_unknown_service() {
        let g = DependencyGraph::new();
        assert!(g.dependencies(&id("ghost")).is_empty());
    }

    #[test]
    fn dependents_of_unknown_service() {
        let g = DependencyGraph::new();
        assert!(g.dependents(&id("ghost")).is_empty());
    }

    #[test]
    fn remove_service_clears_edges() {
        let mut g = DependencyGraph::new();
        g.add_dependency(id("b"), id("a"));
        g.add_dependency(id("c"), id("a"));
        g.remove_service(&id("a"));

        // b and c should have no dependencies now
        assert!(g.dependencies(&id("b")).is_empty());
        assert!(g.dependencies(&id("c")).is_empty());
        assert_eq!(g.len(), 2); // b and c remain
    }

    #[test]
    fn has_missing_dependencies() {
        let mut g = DependencyGraph::new();
        // a depends on "external" which isn't in the graph
        g.deps.entry(id("a")).or_default().insert(id("external"));
        let missing = g.has_missing_dependencies(&id("a"));
        assert_eq!(missing, vec![id("external")]);
    }

    #[test]
    fn no_missing_dependencies() {
        let mut g = DependencyGraph::new();
        g.add_dependency(id("b"), id("a"));
        let missing = g.has_missing_dependencies(&id("b"));
        assert!(missing.is_empty());
    }
}
