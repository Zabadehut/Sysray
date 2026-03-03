use crate::collectors::{async_trait, Collector, Snapshot};
use crate::platform::current;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PressureWindow {
    pub avg10: f64,
    pub avg60: f64,
    pub avg300: f64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PressureMetric {
    pub some: Option<PressureWindow>,
    pub full: Option<PressureWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PsiMetrics {
    pub cpu: PressureMetric,
    pub memory: PressureMetric,
    pub io: PressureMetric,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CgroupMetrics {
    pub version: u8,
    pub path: String,
    pub memory_current_bytes: u64,
    pub memory_max_bytes: Option<u64>,
    pub memory_swap_current_bytes: u64,
    pub memory_swap_max_bytes: Option<u64>,
    pub memory_usage_pct: f64,
    pub pids_current: u64,
    pub pids_max: Option<u64>,
    pub cpu_usage_usec: u64,
    pub cpu_user_usec: u64,
    pub cpu_system_usec: u64,
    pub cpu_nr_periods: u64,
    pub cpu_nr_throttled: u64,
    pub cpu_throttled_usec: u64,
    pub cpu_quota_usec: Option<u64>,
    pub cpu_period_usec: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LinuxMetrics {
    pub timestamp: i64,
    pub cgroup: Option<CgroupMetrics>,
    pub psi: Option<PsiMetrics>,
}

pub struct LinuxCollector;

impl LinuxCollector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Collector for LinuxCollector {
    fn name(&self) -> &'static str {
        "linux"
    }

    async fn collect(&mut self, snapshot: &mut Snapshot) -> Result<()> {
        snapshot.linux = collect_linux_metrics()?;
        Ok(())
    }
}

fn collect_linux_metrics() -> Result<Option<LinuxMetrics>> {
    let linux = current::read_linux_metrics()?;
    if linux.cgroup.is_none() && linux.psi.is_none() {
        return Ok(None);
    }

    Ok(Some(LinuxMetrics {
        timestamp: chrono::Utc::now().timestamp(),
        cgroup: linux.cgroup.map(|cgroup| CgroupMetrics {
            version: cgroup.version,
            path: cgroup.path,
            memory_current_bytes: cgroup.memory_current_bytes,
            memory_max_bytes: cgroup.memory_max_bytes,
            memory_swap_current_bytes: cgroup.memory_swap_current_bytes,
            memory_swap_max_bytes: cgroup.memory_swap_max_bytes,
            memory_usage_pct: cgroup.memory_usage_pct,
            pids_current: cgroup.pids_current,
            pids_max: cgroup.pids_max,
            cpu_usage_usec: cgroup.cpu_usage_usec,
            cpu_user_usec: cgroup.cpu_user_usec,
            cpu_system_usec: cgroup.cpu_system_usec,
            cpu_nr_periods: cgroup.cpu_nr_periods,
            cpu_nr_throttled: cgroup.cpu_nr_throttled,
            cpu_throttled_usec: cgroup.cpu_throttled_usec,
            cpu_quota_usec: cgroup.cpu_quota_usec,
            cpu_period_usec: cgroup.cpu_period_usec,
        }),
        psi: linux.psi.map(|psi| PsiMetrics {
            cpu: PressureMetric {
                some: psi.cpu.some.map(map_pressure_window),
                full: psi.cpu.full.map(map_pressure_window),
            },
            memory: PressureMetric {
                some: psi.memory.some.map(map_pressure_window),
                full: psi.memory.full.map(map_pressure_window),
            },
            io: PressureMetric {
                some: psi.io.some.map(map_pressure_window),
                full: psi.io.full.map(map_pressure_window),
            },
        }),
    }))
}

fn map_pressure_window(window: crate::platform::api::RawPressureWindow) -> PressureWindow {
    PressureWindow {
        avg10: window.avg10,
        avg60: window.avg60,
        avg300: window.avg300,
        total: window.total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn linux_collector_returns_none_or_populated_metrics() {
        let mut collector = LinuxCollector::new();
        let mut snapshot = Snapshot::default();

        collector.collect(&mut snapshot).await.unwrap();

        if let Some(linux) = snapshot.linux {
            assert!(linux.timestamp > 0);
            if let Some(cgroup) = linux.cgroup {
                assert!(cgroup.version >= 2);
            }
        }
    }
}
