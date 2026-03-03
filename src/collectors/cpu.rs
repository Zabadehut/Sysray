use crate::collectors::{async_trait, Collector, Snapshot};
use crate::platform::{api::RawCpuStat, current};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CoreMetrics {
    pub id: usize,
    pub usage_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CpuModeMetrics {
    pub user_pct: f64,
    pub nice_pct: f64,
    pub system_pct: f64,
    pub idle_pct: f64,
    pub iowait_pct: f64,
    pub irq_pct: f64,
    pub softirq_pct: f64,
    pub steal_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CpuMetrics {
    pub timestamp: i64,
    pub global_usage_pct: f64,
    pub per_core: Vec<CoreMetrics>,
    pub load_avg_1: f64,
    pub load_avg_5: f64,
    pub load_avg_15: f64,
    pub context_switches: u64,
    pub interrupts: u64,
    pub steal_pct: f64,
    pub iowait_pct: f64,
    pub modes: CpuModeMetrics,
}

pub struct CpuCollector {
    prev_global: Option<RawCpuStat>,
    prev_cores: Vec<RawCpuStat>,
}

impl CpuCollector {
    pub fn new() -> Self {
        Self {
            prev_global: None,
            prev_cores: Vec::new(),
        }
    }
}

impl Default for CpuCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Collector for CpuCollector {
    fn name(&self) -> &'static str {
        "cpu"
    }

    async fn collect(&mut self, snapshot: &mut Snapshot) -> Result<()> {
        snapshot.cpu = Some(collect_cpu(self)?);
        Ok(())
    }
}

fn delta_modes(cur: &RawCpuStat, prev: &RawCpuStat) -> CpuModeMetrics {
    let dt = cur.total().saturating_sub(prev.total());
    let pct = |n: u64| {
        if dt == 0 {
            0.0
        } else {
            (n as f64 / dt as f64 * 100.0).clamp(0.0, 100.0)
        }
    };
    CpuModeMetrics {
        user_pct: pct(cur.user.saturating_sub(prev.user)),
        nice_pct: pct(cur.nice.saturating_sub(prev.nice)),
        system_pct: pct(cur.system.saturating_sub(prev.system)),
        idle_pct: pct(cur.idle.saturating_sub(prev.idle)),
        iowait_pct: pct(cur.iowait.saturating_sub(prev.iowait)),
        irq_pct: pct(cur.irq.saturating_sub(prev.irq)),
        softirq_pct: pct(cur.softirq.saturating_sub(prev.softirq)),
        steal_pct: pct(cur.steal.saturating_sub(prev.steal)),
    }
}

fn usage_pct_from_modes(modes: &CpuModeMetrics) -> f64 {
    (modes.user_pct
        + modes.nice_pct
        + modes.system_pct
        + modes.irq_pct
        + modes.softirq_pct
        + modes.steal_pct)
        .clamp(0.0, 100.0)
}

fn collect_cpu(c: &mut CpuCollector) -> Result<CpuMetrics> {
    let reading = current::read_cpu()?;

    let modes = if let Some(prev) = c.prev_global.as_ref() {
        delta_modes(&reading.global, prev)
    } else {
        CpuModeMetrics::default()
    };
    let (global_usage_pct, steal_pct, iowait_pct) =
        if let Some(direct_global_usage_pct) = reading.direct_global_usage_pct {
            (
                direct_global_usage_pct,
                reading.direct_steal_pct.unwrap_or(modes.steal_pct),
                reading.direct_iowait_pct.unwrap_or(modes.iowait_pct),
            )
        } else {
            (
                usage_pct_from_modes(&modes),
                modes.steal_pct,
                modes.iowait_pct,
            )
        };

    let per_core = if !reading.direct_per_core_usage_pct.is_empty() {
        reading
            .direct_per_core_usage_pct
            .iter()
            .enumerate()
            .map(|(id, usage_pct)| CoreMetrics {
                id,
                usage_pct: usage_pct.clamp(0.0, 100.0),
            })
            .collect()
    } else {
        reading
            .cores
            .iter()
            .enumerate()
            .map(|(id, stat)| CoreMetrics {
                id,
                usage_pct: c
                    .prev_cores
                    .get(id)
                    .map(|prev| usage_pct_from_modes(&delta_modes(stat, prev)))
                    .unwrap_or(0.0),
            })
            .collect()
    };

    c.prev_global = Some(reading.global.clone());
    c.prev_cores = reading.cores;

    Ok(CpuMetrics {
        timestamp: chrono::Utc::now().timestamp(),
        global_usage_pct,
        per_core,
        load_avg_1: reading.load_avg_1,
        load_avg_5: reading.load_avg_5,
        load_avg_15: reading.load_avg_15,
        context_switches: reading.context_switches,
        interrupts: reading.interrupts,
        steal_pct,
        iowait_pct,
        modes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cpu_collector_populates_snapshot_on_linux() {
        let mut collector = CpuCollector::new();
        let mut snapshot = Snapshot::default();

        collector.collect(&mut snapshot).await.unwrap();

        let cpu = snapshot.cpu.expect("cpu metrics should be present");
        assert!(cpu.global_usage_pct.is_finite());
        assert!(cpu.steal_pct.is_finite());
        assert!(cpu.iowait_pct.is_finite());
        assert!(cpu.modes.user_pct.is_finite());
        assert!(cpu.modes.system_pct.is_finite());
        assert!(cpu.modes.idle_pct.is_finite());
        assert!(cpu.load_avg_1.is_finite());
        assert!(cpu.load_avg_5.is_finite());
        assert!(cpu.load_avg_15.is_finite());

        for (index, core) in cpu.per_core.iter().enumerate() {
            assert_eq!(core.id, index);
            assert!(core.usage_pct.is_finite());
            assert!((0.0..=100.0).contains(&core.usage_pct));
        }
    }
}
