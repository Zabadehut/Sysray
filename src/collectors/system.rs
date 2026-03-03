use crate::collectors::{async_trait, Collector, Snapshot};
use crate::platform::current;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SystemMetrics {
    pub timestamp: i64,
    pub hostname: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub uptime_seconds: u64,
    pub cpu_count: u32,
    pub architecture: String,
}

pub struct SystemCollector;

impl SystemCollector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SystemCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Collector for SystemCollector {
    fn name(&self) -> &'static str {
        "system"
    }

    async fn collect(&mut self, snapshot: &mut Snapshot) -> Result<()> {
        snapshot.system = Some(collect_system()?);
        Ok(())
    }
}

fn collect_system() -> Result<SystemMetrics> {
    let system = current::read_system()?;
    Ok(SystemMetrics {
        timestamp: chrono::Utc::now().timestamp(),
        hostname: system.hostname,
        os_name: system.os_name,
        os_version: system.os_version,
        kernel_version: system.kernel_version,
        uptime_seconds: system.uptime_seconds,
        cpu_count: system.cpu_count,
        architecture: system.architecture,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn system_collector_populates_basic_metadata_on_linux() {
        let mut collector = SystemCollector::new();
        let mut snapshot = Snapshot::default();

        collector.collect(&mut snapshot).await.unwrap();

        let system = snapshot.system.expect("system metrics should be present");
        assert!(!system.os_name.is_empty());
        assert!(!system.architecture.is_empty());
        assert!(system.cpu_count > 0);
    }
}
