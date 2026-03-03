use crate::collectors::{async_trait, Collector, Snapshot};
use crate::platform::{api::RawDiskStat, current};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DiskMetrics {
    pub timestamp: i64,
    pub device: String,
    pub mount_point: String,
    pub total_gb: f64,
    pub used_gb: f64,
    pub free_gb: f64,
    pub usage_pct: f64,
    pub read_iops: u64,
    pub write_iops: u64,
    pub read_throughput_kb: u64,
    pub write_throughput_kb: u64,
    pub await_ms: f64,
    pub service_time_ms: f64,
    pub queue_depth: f64,
    pub util_pct: f64,
    pub read_merged_ops_sec: u64,
    pub write_merged_ops_sec: u64,
}

pub struct DiskCollector {
    prev: HashMap<String, RawDiskStat>,
    prev_at: Option<Instant>,
}

impl DiskCollector {
    pub fn new() -> Self {
        Self {
            prev: HashMap::new(),
            prev_at: None,
        }
    }
}

impl Default for DiskCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Collector for DiskCollector {
    fn name(&self) -> &'static str {
        "disk"
    }

    async fn collect(&mut self, snapshot: &mut Snapshot) -> Result<()> {
        snapshot.disks = collect_disks(self)?;
        Ok(())
    }
}

fn collect_disks(c: &mut DiskCollector) -> Result<Vec<DiskMetrics>> {
    let disks = current::read_disks()?;
    let mount_map = current::read_mount_map();
    let now = chrono::Utc::now().timestamp();
    let collected_at = Instant::now();
    let elapsed_secs = c
        .prev_at
        .map(|prev| collected_at.saturating_duration_since(prev).as_secs_f64())
        .filter(|secs| *secs > 0.0);
    let mut results = Vec::new();

    for disk in disks {
        let (
            read_iops,
            write_iops,
            read_throughput_kb,
            write_throughput_kb,
            await_ms,
            service_time_ms,
            queue_depth,
            util_pct,
            read_merged_ops_sec,
            write_merged_ops_sec,
        ) = if let (Some(prev), Some(elapsed_secs)) = (c.prev.get(&disk.device), elapsed_secs) {
            let dr = disk.reads_completed.saturating_sub(prev.reads_completed);
            let dw = disk.writes_completed.saturating_sub(prev.writes_completed);
            let drm = disk.reads_merged.saturating_sub(prev.reads_merged);
            let dwm = disk.writes_merged.saturating_sub(prev.writes_merged);
            let drb = disk.reads_sectors.saturating_sub(prev.reads_sectors) * 512 / 1024;
            let dwb = disk.writes_sectors.saturating_sub(prev.writes_sectors) * 512 / 1024;
            let drt = disk.reads_time_ms.saturating_sub(prev.reads_time_ms);
            let dwt = disk.writes_time_ms.saturating_sub(prev.writes_time_ms);
            let dt = disk.io_ticks.saturating_sub(prev.io_ticks);
            let dqt = disk
                .weighted_io_time_ms
                .saturating_sub(prev.weighted_io_time_ms);
            (
                per_second_u64(dr, elapsed_secs),
                per_second_u64(dw, elapsed_secs),
                per_second_u64(drb, elapsed_secs),
                per_second_u64(dwb, elapsed_secs),
                compute_await_ms(dr, dw, drt, dwt),
                compute_service_time_ms(dr, dw, dt),
                compute_queue_depth(dqt, elapsed_secs),
                compute_util_pct(dt, elapsed_secs),
                per_second_u64(drm, elapsed_secs),
                per_second_u64(dwm, elapsed_secs),
            )
        } else {
            (0, 0, 0, 0, 0.0, 0.0, 0.0, 0.0, 0, 0)
        };

        let mount_point = mount_map.get(&disk.device).cloned().unwrap_or_default();
        let space = if mount_point.is_empty() {
            crate::platform::api::RawDiskSpace::default()
        } else {
            current::read_disk_space(&mount_point)
        };

        c.prev.insert(disk.device.clone(), disk.clone());

        results.push(DiskMetrics {
            timestamp: now,
            device: disk.device,
            mount_point,
            total_gb: space.total_gb,
            used_gb: space.used_gb,
            free_gb: space.free_gb,
            usage_pct: space.usage_pct,
            read_iops,
            write_iops,
            read_throughput_kb,
            write_throughput_kb,
            await_ms,
            service_time_ms,
            queue_depth,
            util_pct,
            read_merged_ops_sec,
            write_merged_ops_sec,
        });
    }

    c.prev_at = Some(collected_at);
    Ok(results)
}

fn per_second_u64(delta: u64, elapsed_secs: f64) -> u64 {
    if elapsed_secs <= 0.0 {
        0
    } else {
        (delta as f64 / elapsed_secs).round() as u64
    }
}

fn compute_await_ms(read_ios: u64, write_ios: u64, read_time_ms: u64, write_time_ms: u64) -> f64 {
    let total_ios = read_ios + write_ios;
    if total_ios == 0 {
        0.0
    } else {
        (read_time_ms + write_time_ms) as f64 / total_ios as f64
    }
}

fn compute_util_pct(io_ticks_ms: u64, elapsed_secs: f64) -> f64 {
    if elapsed_secs <= 0.0 {
        0.0
    } else {
        (io_ticks_ms as f64 / (elapsed_secs * 1000.0) * 100.0).clamp(0.0, 100.0)
    }
}

fn compute_service_time_ms(read_ios: u64, write_ios: u64, io_ticks_ms: u64) -> f64 {
    let total_ios = read_ios + write_ios;
    if total_ios == 0 {
        0.0
    } else {
        io_ticks_ms as f64 / total_ios as f64
    }
}

fn compute_queue_depth(weighted_io_time_ms: u64, elapsed_secs: f64) -> f64 {
    if elapsed_secs <= 0.0 {
        0.0
    } else {
        weighted_io_time_ms as f64 / (elapsed_secs * 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn await_is_zero_when_no_io_completed() {
        let await_ms = compute_await_ms(0, 0, 25, 30);
        assert_eq!(await_ms, 0.0);
    }

    #[test]
    fn await_uses_total_io_time_over_completed_io() {
        let await_ms = compute_await_ms(4, 6, 20, 40);
        assert_eq!(await_ms, 6.0);
    }

    #[test]
    fn util_scales_with_elapsed_time() {
        let util_pct = compute_util_pct(500, 2.0);
        assert_eq!(util_pct, 25.0);
    }

    #[test]
    fn service_time_uses_busy_time_over_completed_io() {
        let service_time_ms = compute_service_time_ms(5, 5, 40);
        assert_eq!(service_time_ms, 4.0);
    }

    #[test]
    fn queue_depth_scales_with_elapsed_time() {
        let queue_depth = compute_queue_depth(750, 0.5);
        assert_eq!(queue_depth, 1.5);
    }

    #[test]
    fn per_second_scaling_respects_elapsed_time() {
        let per_second = per_second_u64(400, 2.0);
        assert_eq!(per_second, 200);
    }

    #[tokio::test]
    async fn disk_collector_runs_and_produces_sane_values_on_linux() {
        let mut collector = DiskCollector::new();
        let mut snapshot = Snapshot::default();

        collector.collect(&mut snapshot).await.unwrap();

        for disk in snapshot.disks {
            assert!(disk.usage_pct.is_finite());
            assert!((0.0..=100.0).contains(&disk.usage_pct));
            assert!(disk.util_pct.is_finite());
            assert!((0.0..=100.0).contains(&disk.util_pct));
            assert!(disk.await_ms.is_finite());
            assert!(disk.await_ms >= 0.0);
            assert!(disk.service_time_ms.is_finite());
            assert!(disk.service_time_ms >= 0.0);
            assert!(disk.queue_depth.is_finite());
            assert!(disk.queue_depth >= 0.0);
            assert!(disk.total_gb >= 0.0);
            assert!(disk.used_gb >= 0.0);
            assert!(disk.free_gb >= 0.0);
        }
    }
}
