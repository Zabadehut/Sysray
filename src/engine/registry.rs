use crate::collectors::{Collector, Snapshot};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Santé d'un collector — exposée via /health.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CollectorHealth {
    pub run_count: u64,
    pub error_count: u64,
    pub last_error: Option<String>,
    pub last_ok_ts: Option<i64>,
}

/// Slot interne : collector + intervalle de collecte + état d'exécution.
/// L'intervalle vient du config — le collector ne le connaît pas.
pub struct CollectorSlot {
    pub collector: Box<dyn Collector>,
    pub interval: Duration,
    pub health: CollectorHealth,
    pub next_run: Instant,
}

impl CollectorSlot {
    pub fn new(collector: Box<dyn Collector>, interval: Duration) -> Self {
        Self {
            next_run: Instant::now(),
            health: CollectorHealth::default(),
            interval,
            collector,
        }
    }

    pub fn name(&self) -> &'static str {
        self.collector.name()
    }
}

/// Registre central : liste des collectors actifs + snapshot partagé.
pub struct Registry {
    pub slots: Vec<CollectorSlot>,
    pub latest: Arc<RwLock<Snapshot>>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            latest: Arc::new(RwLock::new(Snapshot::default())),
        }
    }

    pub fn register<C: Collector + 'static>(&mut self, collector: C, interval: Duration) {
        self.slots
            .push(CollectorSlot::new(Box::new(collector), interval));
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}
