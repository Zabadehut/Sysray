use crate::collectors::Snapshot;
use crate::exporters::Exporter;
use anyhow::Result;

pub struct JsonExporter;

impl Exporter for JsonExporter {
    fn name(&self) -> &'static str {
        "json"
    }

    fn export(&self, snapshot: &Snapshot) -> Result<String> {
        Ok(serde_json::to_string_pretty(snapshot)?)
    }
}
