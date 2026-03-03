pub mod csv;
pub mod json;
pub mod prometheus;

use crate::collectors::Snapshot;
use anyhow::Result;

pub trait Exporter: Send + Sync {
    fn name(&self) -> &'static str;
    fn export(&self, snapshot: &Snapshot) -> Result<String>;
}
