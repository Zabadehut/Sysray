use crate::collectors::Snapshot;
use crate::pipeline::PipelineStage;

pub struct MemoryPressureStage;

impl MemoryPressureStage {
    pub fn new() -> Self {
        Self
    }
}

impl PipelineStage for MemoryPressureStage {
    fn name(&self) -> &'static str {
        "mem_pressure"
    }

    fn process(&mut self, snapshot: &mut Snapshot) {
        let Some(memory) = snapshot.memory.as_ref() else {
            return;
        };

        let mem_ratio = if memory.total_kb > 0 {
            memory.used_kb as f64 / memory.total_kb as f64
        } else {
            0.0
        };

        let swap_ratio = if memory.swap_total_kb > 0 {
            memory.swap_used_kb as f64 / memory.swap_total_kb as f64
        } else {
            0.0
        };

        snapshot.computed.memory_pressure = (mem_ratio * 0.7 + swap_ratio * 0.3).clamp(0.0, 1.0);
    }
}
