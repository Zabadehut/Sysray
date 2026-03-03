use crate::collectors::Snapshot;
use crate::pipeline::PipelineStage;
use std::collections::VecDeque;

pub struct CpuTrendStage {
    samples: VecDeque<f64>,
    capacity: usize,
}

impl CpuTrendStage {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(capacity),
            capacity,
        }
    }
}

impl PipelineStage for CpuTrendStage {
    fn name(&self) -> &'static str {
        "cpu_trend"
    }

    fn process(&mut self, snapshot: &mut Snapshot) {
        let Some(cpu) = snapshot.cpu.as_ref() else {
            return;
        };

        if self.samples.len() == self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(cpu.global_usage_pct);

        if self.samples.is_empty() {
            return;
        }

        let mut values: Vec<f64> = self.samples.iter().copied().collect();
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        snapshot.computed.cpu_trend_p50 = percentile(&values, 0.50);
        snapshot.computed.cpu_trend_p95 = percentile(&values, 0.95);
    }
}

fn percentile(values: &[f64], ratio: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let index = ((values.len() - 1) as f64 * ratio).round() as usize;
    values[index.min(values.len() - 1)]
}
