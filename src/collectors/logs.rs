use crate::collectors::{async_trait, Collector, LogsMetrics, Snapshot};
use crate::config::LogsConfig;
use crate::log_sources;
use anyhow::Result;

pub struct LogsCollector {
    config: LogsConfig,
}

impl LogsCollector {
    pub fn new(config: LogsConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Collector for LogsCollector {
    fn name(&self) -> &'static str {
        "logs"
    }

    async fn collect(&mut self, snapshot: &mut Snapshot) -> Result<()> {
        if !self.config.enabled {
            snapshot.logs = None;
            return Ok(());
        }

        snapshot.logs = Some(LogsMetrics {
            timestamp: chrono::Utc::now().timestamp(),
            system_events: log_sources::read_system_events(
                self.config.system_window_secs,
                self.config.system_max_entries,
            ),
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn logs_collector_populates_snapshot_without_panicking() {
        let mut collector = LogsCollector::new(LogsConfig::default());
        let mut snapshot = Snapshot::default();
        collector.collect(&mut snapshot).await.unwrap();
        assert!(snapshot.logs.is_some());
    }
}
