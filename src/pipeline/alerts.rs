use crate::collectors::{Alert, AlertLevel, Snapshot};
use crate::config::AlertThresholds;
use crate::pipeline::PipelineStage;

pub struct AlertStage {
    thresholds: AlertThresholds,
}

impl AlertStage {
    pub fn new(thresholds: AlertThresholds) -> Self {
        Self { thresholds }
    }
}

impl PipelineStage for AlertStage {
    fn name(&self) -> &'static str {
        "alerts"
    }

    fn process(&mut self, snapshot: &mut Snapshot) {
        let mut alerts = Vec::new();

        if let Some(cpu) = snapshot.cpu.as_ref() {
            if cpu.global_usage_pct >= self.thresholds.cpu_critical_pct {
                alerts.push(Alert {
                    level: AlertLevel::Critical,
                    message: format!("CPU usage critical at {:.1}%", cpu.global_usage_pct),
                });
            } else if cpu.global_usage_pct >= self.thresholds.cpu_warning_pct {
                alerts.push(Alert {
                    level: AlertLevel::Warning,
                    message: format!("CPU usage high at {:.1}%", cpu.global_usage_pct),
                });
            }
        }

        if let Some(memory) = snapshot.memory.as_ref() {
            if memory.usage_pct >= self.thresholds.mem_critical_pct {
                alerts.push(Alert {
                    level: AlertLevel::Critical,
                    message: format!("Memory usage critical at {:.1}%", memory.usage_pct),
                });
            } else if memory.usage_pct >= self.thresholds.mem_warning_pct {
                alerts.push(Alert {
                    level: AlertLevel::Warning,
                    message: format!("Memory usage high at {:.1}%", memory.usage_pct),
                });
            }
        }

        if let Some(linux) = snapshot.linux.as_ref() {
            if let Some(cgroup) = linux.cgroup.as_ref() {
                if cgroup.memory_usage_pct >= self.thresholds.cgroup_memory_critical_pct {
                    alerts.push(Alert {
                        level: AlertLevel::Critical,
                        message: format!(
                            "Cgroup memory critical at {:.1}% of limit",
                            cgroup.memory_usage_pct
                        ),
                    });
                } else if cgroup.memory_usage_pct >= self.thresholds.cgroup_memory_warning_pct {
                    alerts.push(Alert {
                        level: AlertLevel::Warning,
                        message: format!(
                            "Cgroup memory high at {:.1}% of limit",
                            cgroup.memory_usage_pct
                        ),
                    });
                }

                let throttling_pct = cgroup_cpu_throttling_pct(cgroup);
                if throttling_pct >= self.thresholds.cgroup_cpu_throttling_critical_pct {
                    alerts.push(Alert {
                        level: AlertLevel::Critical,
                        message: format!(
                            "Cgroup CPU throttling critical at {:.1}% of periods",
                            throttling_pct
                        ),
                    });
                } else if throttling_pct >= self.thresholds.cgroup_cpu_throttling_warning_pct {
                    alerts.push(Alert {
                        level: AlertLevel::Warning,
                        message: format!(
                            "Cgroup CPU throttling high at {:.1}% of periods",
                            throttling_pct
                        ),
                    });
                }
            }

            if let Some(psi) = linux.psi.as_ref() {
                push_psi_alert(
                    &mut alerts,
                    "CPU",
                    psi.cpu.some.as_ref().map(|window| window.avg10),
                    self.thresholds.psi_cpu_some_warning_pct,
                    self.thresholds.psi_cpu_some_critical_pct,
                );
                push_psi_alert(
                    &mut alerts,
                    "Memory",
                    psi.memory.some.as_ref().map(|window| window.avg10),
                    self.thresholds.psi_memory_some_warning_pct,
                    self.thresholds.psi_memory_some_critical_pct,
                );
                push_psi_alert(
                    &mut alerts,
                    "IO",
                    psi.io.some.as_ref().map(|window| window.avg10),
                    self.thresholds.psi_io_some_warning_pct,
                    self.thresholds.psi_io_some_critical_pct,
                );
            }
        }

        let mut info = 0usize;
        let mut warning = 0usize;
        let mut critical = 0usize;

        for alert in &alerts {
            match alert.level {
                AlertLevel::Info => info += 1,
                AlertLevel::Warning => warning += 1,
                AlertLevel::Critical => critical += 1,
            }
        }

        snapshot.computed.alerts_info = info;
        snapshot.computed.alerts_warning = warning;
        snapshot.computed.alerts_critical = critical;
        snapshot.computed.alerts = alerts;
    }
}

fn cgroup_cpu_throttling_pct(cgroup: &crate::collectors::linux::CgroupMetrics) -> f64 {
    if cgroup.cpu_nr_periods == 0 {
        0.0
    } else {
        (cgroup.cpu_nr_throttled as f64 / cgroup.cpu_nr_periods as f64 * 100.0).clamp(0.0, 100.0)
    }
}

fn push_psi_alert(
    alerts: &mut Vec<Alert>,
    resource: &str,
    avg10: Option<f64>,
    warning_threshold: f64,
    critical_threshold: f64,
) {
    let Some(avg10) = avg10 else {
        return;
    };

    if avg10 >= critical_threshold {
        alerts.push(Alert {
            level: AlertLevel::Critical,
            message: format!("{} PSI critical at avg10 {:.1}%", resource, avg10),
        });
    } else if avg10 >= warning_threshold {
        alerts.push(Alert {
            level: AlertLevel::Warning,
            message: format!("{} PSI high at avg10 {:.1}%", resource, avg10),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::linux::{
        CgroupMetrics, LinuxMetrics, PressureMetric, PressureWindow, PsiMetrics,
    };

    #[test]
    fn alerts_include_linux_pressure_and_cgroup_signals() {
        let mut snapshot = Snapshot {
            linux: Some(LinuxMetrics {
                timestamp: 1,
                cgroup: Some(CgroupMetrics {
                    memory_usage_pct: 96.0,
                    cpu_nr_periods: 100,
                    cpu_nr_throttled: 30,
                    ..CgroupMetrics::default()
                }),
                psi: Some(PsiMetrics {
                    cpu: PressureMetric {
                        some: Some(PressureWindow {
                            avg10: 22.0,
                            ..PressureWindow::default()
                        }),
                        full: None,
                    },
                    memory: PressureMetric {
                        some: Some(PressureWindow {
                            avg10: 11.0,
                            ..PressureWindow::default()
                        }),
                        full: None,
                    },
                    io: PressureMetric {
                        some: Some(PressureWindow {
                            avg10: 21.0,
                            ..PressureWindow::default()
                        }),
                        full: None,
                    },
                }),
            }),
            ..Snapshot::default()
        };

        let mut stage = AlertStage::new(AlertThresholds::default());
        stage.process(&mut snapshot);

        assert!(snapshot
            .computed
            .alerts
            .iter()
            .any(|alert| alert.message.contains("Cgroup memory critical")));
        assert!(snapshot
            .computed
            .alerts
            .iter()
            .any(|alert| alert.message.contains("Cgroup CPU throttling critical")));
        assert!(snapshot
            .computed
            .alerts
            .iter()
            .any(|alert| alert.message.contains("CPU PSI critical")));
        assert!(snapshot
            .computed
            .alerts
            .iter()
            .any(|alert| alert.message.contains("Memory PSI critical")));
        assert!(snapshot
            .computed
            .alerts
            .iter()
            .any(|alert| alert.message.contains("IO PSI critical")));
        assert_eq!(snapshot.computed.alerts_info, 0);
        assert_eq!(snapshot.computed.alerts_warning, 0);
        assert!(snapshot.computed.alerts_critical >= 5);
    }
}
