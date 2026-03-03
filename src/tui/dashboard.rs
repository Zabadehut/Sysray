use crate::collectors::Snapshot;
use crate::tui::{
    theme::Theme,
    widgets::{
        alerts_widget, cpu_widget, disk_widget, linux_widget, memory_widget, network_widget,
        process_widget,
    },
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

#[derive(Debug, Clone, Copy)]
pub enum Panel {
    Cpu,
    Memory,
    Linux,
    Disk,
    Network,
    Alerts,
    Process,
}

#[derive(Debug, Clone, Copy)]
struct PanelVisibility {
    cpu: bool,
    memory: bool,
    linux: bool,
    disk: bool,
    network: bool,
    alerts: bool,
    process: bool,
}

impl Default for PanelVisibility {
    fn default() -> Self {
        Self {
            cpu: true,
            memory: true,
            linux: true,
            disk: true,
            network: true,
            alerts: true,
            process: true,
        }
    }
}

impl PanelVisibility {
    fn toggle(&mut self, panel: Panel) {
        match panel {
            Panel::Cpu => self.cpu = !self.cpu,
            Panel::Memory => self.memory = !self.memory,
            Panel::Linux => self.linux = !self.linux,
            Panel::Disk => self.disk = !self.disk,
            Panel::Network => self.network = !self.network,
            Panel::Alerts => self.alerts = !self.alerts,
            Panel::Process => self.process = !self.process,
        }
    }

    fn is_visible(&self, panel: Panel) -> bool {
        match panel {
            Panel::Cpu => self.cpu,
            Panel::Memory => self.memory,
            Panel::Linux => self.linux,
            Panel::Disk => self.disk,
            Panel::Network => self.network,
            Panel::Alerts => self.alerts,
            Panel::Process => self.process,
        }
    }

    fn visible_count(&self) -> usize {
        [
            self.cpu,
            self.memory,
            self.linux,
            self.disk,
            self.network,
            self.alerts,
            self.process,
        ]
        .into_iter()
        .filter(|visible| *visible)
        .count()
    }
}

pub struct Dashboard {
    pub theme_name: String,
    pub theme: Theme,
    visibility: PanelVisibility,
}

impl Dashboard {
    pub fn new(theme_name: &str) -> Self {
        let theme_name = Theme::normalize_name(theme_name).to_string();
        Self {
            theme: Theme::from_name(&theme_name),
            theme_name,
            visibility: PanelVisibility::default(),
        }
    }

    pub fn cycle_theme(&mut self) {
        self.theme_name = Theme::next_name(&self.theme_name).to_string();
        self.theme = Theme::from_name(&self.theme_name);
    }

    pub fn toggle_panel(&mut self, panel: Panel) {
        self.visibility.toggle(panel);
    }

    pub fn render(&self, frame: &mut Frame, snapshot: &Snapshot) {
        let area = frame.area();

        // Layout principal : header + corps + footer
        let main = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // header
                Constraint::Min(0),    // corps
                Constraint::Length(1), // footer
            ])
            .split(area);

        self.render_header(frame, main[0], snapshot);
        self.render_body(frame, main[1], snapshot);
        self.render_footer(frame, main[2], snapshot);
    }

    fn render_header(&self, frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
        let hostname = snapshot
            .system
            .as_ref()
            .map(|s| s.hostname.as_str())
            .unwrap_or("unknown");

        let uptime = snapshot
            .system
            .as_ref()
            .map(|s| format_uptime(s.uptime_seconds))
            .unwrap_or_default();

        let os = snapshot
            .system
            .as_ref()
            .map(|s| s.os_name.clone())
            .unwrap_or_default();

        let ts = chrono::DateTime::from_timestamp(snapshot.timestamp, 0)
            .map(|dt| dt.format("%H:%M:%S").to_string())
            .unwrap_or_default();

        let header = Paragraph::new(Line::from(vec![
            Span::styled(" ◉ PULSAR ", self.theme.title_style()),
            Span::raw(format!("  {}  {}  {}  {}", hostname, os, uptime, ts)),
        ]));
        frame.render_widget(header, area);
    }

    fn render_body(&self, frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
        let left_panels = [Panel::Cpu, Panel::Memory, Panel::Linux];
        let right_panels = [Panel::Disk, Panel::Network, Panel::Alerts];

        let has_left = left_panels
            .into_iter()
            .any(|panel| self.visibility.is_visible(panel));
        let has_right = right_panels
            .into_iter()
            .any(|panel| self.visibility.is_visible(panel));
        let has_top = has_left || has_right;
        let has_process = self.visibility.is_visible(Panel::Process);

        match (has_top, has_process) {
            (true, true) => {
                let rows = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(25), Constraint::Min(0)])
                    .split(area);
                self.render_top(frame, rows[0], snapshot, has_left, has_right);
                process_widget::render(frame, rows[1], &snapshot.processes, &self.theme);
            }
            (true, false) => self.render_top(frame, area, snapshot, has_left, has_right),
            (false, true) => process_widget::render(frame, area, &snapshot.processes, &self.theme),
            (false, false) => {
                frame.render_widget(
                    Paragraph::new("All panels hidden. Toggle with c/m/l/d/n/a/p."),
                    area,
                );
            }
        }
    }

    fn render_top(
        &self,
        frame: &mut Frame,
        area: Rect,
        snapshot: &Snapshot,
        has_left: bool,
        has_right: bool,
    ) {
        match (has_left, has_right) {
            (true, true) => {
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                    .split(area);
                self.render_left_stack(frame, cols[0], snapshot);
                self.render_right_stack(frame, cols[1], snapshot);
            }
            (true, false) => self.render_left_stack(frame, area, snapshot),
            (false, true) => self.render_right_stack(frame, area, snapshot),
            (false, false) => {}
        }
    }

    fn render_left_stack(&self, frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
        let panels = self.visible_panels(&[Panel::Cpu, Panel::Memory, Panel::Linux]);
        let chunks = split_vertical(area, panels.len());

        for (panel, chunk) in panels.into_iter().zip(chunks.into_iter()) {
            match panel {
                Panel::Cpu => cpu_widget::render(
                    frame,
                    chunk,
                    snapshot.cpu.as_ref(),
                    snapshot.computed.cpu_trend_p50,
                    snapshot.computed.cpu_trend_p95,
                    &self.theme,
                ),
                Panel::Memory => memory_widget::render(
                    frame,
                    chunk,
                    snapshot.memory.as_ref(),
                    snapshot.computed.memory_pressure,
                    &self.theme,
                ),
                Panel::Linux => {
                    linux_widget::render(frame, chunk, snapshot.linux.as_ref(), &self.theme)
                }
                _ => {}
            }
        }
    }

    fn render_right_stack(&self, frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
        let panels = self.visible_panels(&[Panel::Disk, Panel::Network, Panel::Alerts]);
        let chunks = split_vertical(area, panels.len());

        for (panel, chunk) in panels.into_iter().zip(chunks.into_iter()) {
            match panel {
                Panel::Disk => disk_widget::render(frame, chunk, &snapshot.disks, &self.theme),
                Panel::Network => {
                    network_widget::render(frame, chunk, &snapshot.networks, &self.theme)
                }
                Panel::Alerts => {
                    alerts_widget::render(frame, chunk, &snapshot.computed.alerts, &self.theme)
                }
                _ => {}
            }
        }
    }

    fn visible_panels(&self, panels: &[Panel]) -> Vec<Panel> {
        panels
            .iter()
            .copied()
            .filter(|panel| self.visibility.is_visible(*panel))
            .collect()
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect, snapshot: &Snapshot) {
        let alerts = snapshot.computed.alerts.len();
        let visibility = &self.visibility;
        let footer = Paragraph::new(Line::from(vec![
            Span::styled(" q", self.theme.highlight_style()),
            Span::raw(":quit  "),
            Span::styled("r", self.theme.highlight_style()),
            Span::raw(":refresh  "),
            Span::styled("t", self.theme.highlight_style()),
            Span::raw(format!(":theme({})  ", self.theme_name)),
            panel_toggle_span("c", "cpu", visibility.cpu, &self.theme),
            Span::raw(" "),
            panel_toggle_span("m", "mem", visibility.memory, &self.theme),
            Span::raw(" "),
            panel_toggle_span("l", "linux", visibility.linux, &self.theme),
            Span::raw(" "),
            panel_toggle_span("d", "disk", visibility.disk, &self.theme),
            Span::raw(" "),
            panel_toggle_span("n", "net", visibility.network, &self.theme),
            Span::raw(" "),
            panel_toggle_span("a", "alerts", visibility.alerts, &self.theme),
            Span::raw(" "),
            panel_toggle_span("p", "proc", visibility.process, &self.theme),
            Span::raw(format!("  visible:{}/7  ", self.visibility.visible_count())),
            Span::styled(
                format!(
                    "alerts:{} w:{} c:{}  ",
                    alerts, snapshot.computed.alerts_warning, snapshot.computed.alerts_critical
                ),
                if alerts > 0 {
                    self.theme.alert_style()
                } else {
                    self.theme.highlight_style()
                },
            ),
            Span::raw("  Pulsar v0.1.0 — Kevin Vanden-Brande"),
        ]));
        frame.render_widget(footer, area);
    }
}

fn split_vertical(area: Rect, count: usize) -> Vec<Rect> {
    if count == 0 {
        return Vec::new();
    }
    if count == 1 {
        return vec![area];
    }

    let constraints = vec![Constraint::Ratio(1, count as u32); count];
    Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area)
        .iter()
        .copied()
        .collect()
}

fn panel_toggle_span<'a>(key: &'a str, label: &'a str, visible: bool, theme: &Theme) -> Span<'a> {
    let text = if visible {
        format!("{key}:{label}")
    } else {
        format!("{key}:{label} off")
    };

    Span::styled(
        text,
        if visible {
            theme.highlight_style()
        } else {
            theme.muted_style()
        },
    )
}

fn format_uptime(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    if days > 0 {
        format!("up {}d {}h {}m", days, hours, mins)
    } else if hours > 0 {
        format!("up {}h {}m", hours, mins)
    } else {
        format!("up {}m", mins)
    }
}
