use crate::collectors::Snapshot;
use crate::engine::registry::{CollectorHealth, Registry};
use crate::pipeline::PipelineRunner;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

/// Tick émis à chaque fois qu'au moins un collector a produit de nouvelles données.
#[derive(Clone, Debug)]
pub struct TickEvent {
    pub snapshot: Snapshot,
}

/// Cadence de base du scheduler : fréquence à laquelle il vérifie
/// quels collectors sont prêts. Doit être ≤ min(collector.interval()).
const BASE_TICK: Duration = Duration::from_millis(500);

/// Orchestre les collectors à leurs intervalles propres et diffuse
/// chaque snapshot mis à jour via un broadcast channel.
pub struct Scheduler {
    registry: Registry,
    pipeline: PipelineRunner,
    tx: broadcast::Sender<TickEvent>,
    latest: Arc<RwLock<Snapshot>>,
    /// Santé de chaque collector — accessible via health() pour l'API.
    health: Arc<RwLock<HashMap<String, CollectorHealth>>>,
}

impl Scheduler {
    pub fn new(
        registry: Registry,
        pipeline: PipelineRunner,
    ) -> (Self, broadcast::Receiver<TickEvent>) {
        let latest = Arc::clone(&registry.latest);
        let (tx, rx) = broadcast::channel(64);

        // Pré-peupler la map de santé avec les noms de collectors
        let mut health_map = HashMap::new();
        for slot in &registry.slots {
            health_map.insert(slot.name().to_string(), CollectorHealth::default());
        }
        let health = Arc::new(RwLock::new(health_map));

        (
            Self {
                registry,
                pipeline,
                tx,
                latest,
                health,
            },
            rx,
        )
    }

    /// Arc partagé vers le dernier snapshot (pour polling HTTP).
    pub fn latest(&self) -> Arc<RwLock<Snapshot>> {
        Arc::clone(&self.latest)
    }

    /// Arc partagé vers la santé des collectors (pour /health API).
    pub fn health(&self) -> Arc<RwLock<HashMap<String, CollectorHealth>>> {
        Arc::clone(&self.health)
    }

    /// Démarre la boucle principale. S'arrête quand le token est annulé.
    pub async fn run(mut self, token: CancellationToken) {
        info!(
            collectors = self.registry.slots.len(),
            base_tick_ms = BASE_TICK.as_millis(),
            "Scheduler started"
        );

        // Tous les collectors partent immédiatement au premier tick.
        // /proc est une lecture mémoire — il n'y a pas de spike I/O à craindre.

        let mut ticker = tokio::time::interval(BASE_TICK);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = ticker.tick() => self.tick().await,
                _ = token.cancelled() => {
                    info!("Scheduler stopping.");
                    break;
                }
            }
        }
    }

    async fn tick(&mut self) {
        let now = Instant::now();
        let timestamp = Utc::now().timestamp();

        let mut snapshot = self.latest.read().await.clone();
        snapshot.timestamp = timestamp;

        let mut updated = false;

        for slot in &mut self.registry.slots {
            if now < slot.next_run {
                continue;
            }

            let name = slot.name().to_string();

            match slot.collector.collect(&mut snapshot).await {
                Ok(()) => {
                    slot.health.run_count += 1;
                    slot.health.last_ok_ts = Some(timestamp);
                    slot.health.last_error = None;
                    updated = true;
                }
                Err(e) => {
                    slot.health.error_count += 1;
                    slot.health.last_error = Some(e.to_string());
                    error!(collector = %name, error = %e, "Collector failed");
                }
            }

            // Synchroniser la santé dans l'Arc partagé
            self.health.write().await.insert(name, slot.health.clone());

            slot.next_run = now + slot.interval;
        }

        if updated {
            self.pipeline.run(&mut snapshot);
            *self.latest.write().await = snapshot.clone();
            let _ = self.tx.send(TickEvent { snapshot });
        }
    }
}
