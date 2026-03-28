use crate::service::{ServiceId, ServiceDescriptor, ServiceState};
use std::collections::{HashMap, VecDeque};

/// Per-service runtime state
#[derive(Debug)]
pub struct ServiceEntry {
    pub descriptor: ServiceDescriptor,
    pub state: ServiceState,
    pub pid: Option<u32>,
    pub restart_count: u32,
    pub last_start: Option<std::time::Instant>,
    pub last_exit_code: Option<i32>,
    pub error: Option<String>,
}

/// Central service registry
pub struct ServiceRegistry {
    services: HashMap<ServiceId, ServiceEntry>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self { services: HashMap::new() }
    }

    pub fn register(&mut self, desc: ServiceDescriptor) {
        let id = desc.id.clone();
        self.services.insert(id, ServiceEntry {
            descriptor: desc,
            state: ServiceState::Stopped,
            pid: None,
            restart_count: 0,
            last_start: None,
            last_exit_code: None,
            error: None,
        });
    }

    pub fn get(&self, id: &ServiceId) -> Option<&ServiceEntry> {
        self.services.get(id)
    }

    pub fn get_mut(&mut self, id: &ServiceId) -> Option<&mut ServiceEntry> {
        self.services.get_mut(id)
    }

    pub fn all_services(&self) -> Vec<&ServiceEntry> {
        self.services.values().collect()
    }

    /// Topological sort: returns service IDs in startup order (dependencies first)
    pub fn startup_order(&self) -> Result<Vec<ServiceId>, CycleError> {
        let mut in_degree: HashMap<&ServiceId, usize> = HashMap::new();
        let mut graph: HashMap<&ServiceId, Vec<&ServiceId>> = HashMap::new();

        // Initialize
        for id in self.services.keys() {
            in_degree.entry(id).or_insert(0);
            graph.entry(id).or_default();
        }

        // Build edges
        for (id, entry) in &self.services {
            for dep in &entry.descriptor.depends_on {
                if self.services.contains_key(dep) {
                    graph.entry(dep).or_default().push(id);
                    *in_degree.entry(id).or_insert(0) += 1;
                }
            }
        }

        // Kahn's algorithm
        let mut queue: VecDeque<&ServiceId> = in_degree.iter()
            .filter(|&(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();

        // Sort queue by priority for deterministic ordering
        let mut queue_vec: Vec<&ServiceId> = queue.drain(..).collect();
        queue_vec.sort_by_key(|id| {
            self.services.get(*id).map(|e| e.descriptor.priority).unwrap_or(i32::MAX)
        });
        queue = queue_vec.into_iter().collect();

        let mut result = Vec::new();

        while let Some(node) = queue.pop_front() {
            result.push(node.clone());

            if let Some(dependents) = graph.get(node) {
                for dep in dependents {
                    if let Some(deg) = in_degree.get_mut(dep) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dep);
                        }
                    }
                }
            }
        }

        if result.len() != self.services.len() {
            // Find cycle
            let missing: Vec<ServiceId> = self.services.keys()
                .filter(|id| !result.contains(id))
                .cloned()
                .collect();
            Err(CycleError { services: missing })
        } else {
            Ok(result)
        }
    }

    /// Reverse topological sort: shutdown order (dependents first)
    pub fn shutdown_order(&self) -> Result<Vec<ServiceId>, CycleError> {
        let mut order = self.startup_order()?;
        order.reverse();
        Ok(order)
    }

    /// Get all services that depend on the given service
    pub fn dependents(&self, id: &ServiceId) -> Vec<ServiceId> {
        self.services.iter()
            .filter(|(_, entry)| entry.descriptor.depends_on.contains(id))
            .map(|(sid, _)| sid.clone())
            .collect()
    }

    /// Get auto-start services
    pub fn auto_start_services(&self) -> Vec<ServiceId> {
        self.services.iter()
            .filter(|(_, entry)| entry.descriptor.auto_start && entry.state == ServiceState::Stopped)
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn set_state(&mut self, id: &ServiceId, state: ServiceState) {
        if let Some(entry) = self.services.get_mut(id) {
            entry.state = state;
        }
    }

    pub fn service_count(&self) -> usize {
        self.services.len()
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self { Self::new() }
}

#[derive(Debug)]
pub struct CycleError {
    pub services: Vec<ServiceId>,
}

impl std::fmt::Display for CycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dependency cycle among: {:?}", self.services.iter().map(|s| &s.0).collect::<Vec<_>>())
    }
}
impl std::error::Error for CycleError {}
