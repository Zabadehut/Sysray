mod alerts;
mod cpu_trend;
mod mem_pressure;

use crate::collectors::Snapshot;

pub use alerts::AlertStage;
pub use cpu_trend::CpuTrendStage;
pub use mem_pressure::MemoryPressureStage;

pub trait PipelineStage: Send + Sync {
    fn name(&self) -> &'static str;
    fn process(&mut self, snapshot: &mut Snapshot);
}

#[derive(Default)]
pub struct PipelineRunner {
    stages: Vec<Box<dyn PipelineStage>>,
}

impl PipelineRunner {
    pub fn new(stages: Vec<Box<dyn PipelineStage>>) -> Self {
        Self { stages }
    }

    pub fn run(&mut self, snapshot: &mut Snapshot) {
        for stage in &mut self.stages {
            let _ = stage.name();
            stage.process(snapshot);
        }
    }
}
