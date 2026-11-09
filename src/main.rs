use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::{io, time::{Duration, Instant}};
use sysinfo::{Disks, System};

struct App {
    tab: usize,
    should_quit: bool,
    system: System,
    disks: Disks,
    refreshed: Instant,
    host: String,
    os: String,
    findings: Vec<String>,
}

impl App {
    fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        let disks = Disks::new_with_refreshed_list();
        let host = System::host_name().unwrap_or_else(|| "Unknown host".into());
        let os = format!(
            "{} {} (kernel {})",
            System::name().unwrap_or_else(|| "Windows".into()),
            System::os_version().unwrap_or_else(|| "version unknown".into()),
            System::kernel_version().unwrap_or_else(|| "unknown".into())
        );
        Self {
            tab: 0, should_quit: false, system, disks, refreshed: Instant::now(),
            host, os, findings: vec![
                "No diagnostic scans have been run yet.".into(),
                "Live inventory is informational; it is not a full health verdict.".into(),
            ],
        }
    }

    fn refresh(&mut self) {
        self.system.refresh_all();
        self.disks.refresh(true);
        self.refreshed = Instant::now();
    }

    fn cpu_usage(&self) -> f32 {
        let cpus = self.system.cpus();
        if cpus.is_empty() { 0.0 } else {
            cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / cpus.len() as f32
        }
    }
}

struct TerminalGuard;
impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode().context("Could not enable terminal raw mode")?;
        execute!(io::stdout(), EnterAlternateScreen).context("Could not enter alternate screen")?;
        Ok(Self)
    }
}
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

fn main() -> Result<()> {
    let _guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new();
    while !app.should_quit {
        terminal.draw(|f| draw(f, &app))?;
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press { continue; }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                    KeyCode::Right | KeyCode::Tab => app.tab = (app.tab + 1) % 4,
                    KeyCode::Left => app.tab = (app.tab + 3) % 4,
                    KeyCode::Char('1') => app.tab = 0,
                    KeyCode::Char('2') => app.tab = 1,
                    KeyCode::Char('3') => app.tab = 2,
                    KeyCode::Char('4') => app.tab = 3,
                    KeyCode::Char('r') => app.refresh(),
                    _ => {}
                }
            }
        }
        if app.refreshed.elapsed() >= Duration::from_secs(2) { app.refresh(); }
    }
    Ok(())
}

fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(8), Constraint::Length(2)])
        .split(area);
    let tabs = ["Overview", "Diagnostics", "Storage", "About"];
    let mut spans = vec![Span::styled(" SYSTEMWATCH ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))];
    for (i, tab) in tabs.iter().enumerate() {
        let style = if app.tab == i { Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Gray) };
        spans.push(Span::styled(format!("  {}:{}  ", i + 1, tab), style));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)).block(Block::default().borders(Borders::ALL).title(" Portable Windows diagnostics ")), chunks[0]);
    match app.tab {
        0 => draw_overview(frame, app, chunks[1]),
        1 => draw_diagnostics(frame, app, chunks[1]),
        2 => draw_storage(frame, app, chunks[1]),
        _ => draw_about(frame, chunks[1]),
    }
    frame.render_widget(Paragraph::new(" в†ђ/в†’ or 1-4: navigate   R: refresh   Q/Esc: quit   вЂў   Read-only preview"),
        chunks[2]);
}

fn draw_overview(frame: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Length(7), Constraint::Min(5)])
        .split(area);
    let cpu = app.cpu_usage();
    let mem_total = app.system.total_memory();
    let mem_used = app.system.used_memory();
    let mem_pct = if mem_total == 0 { 0.0 } else { mem_used as f64 / mem_total as f64 * 100.0 };
    let summary = format!("Host: {}\nOS: {}\nCPU: {} logical processors",
        app.host, app.os, app.system.cpus().len());
    frame.render_widget(Paragraph::new(summary).block(Block::default().borders(Borders::ALL).title(" System ")).wrap(Wrap { trim: true }), rows[0]);

    let metrics = Layout::default().direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)]).split(rows[1]);
    frame.render_widget(Gauge::default().block(Block::default().borders(Borders::ALL).title(" CPU average "))
        .gauge_style(Style::default().fg(Color::Cyan)).percent(cpu.clamp(0.0, 100.0) as u16)
        .label(format!("{:.1}%", cpu)), metrics[0]);
    frame.render_widget(Gauge::default().block(Block::default().borders(Borders::ALL).title(" Memory "))
        .gauge_style(Style::default().fg(Color::Yellow)).percent(mem_pct.clamp(0.0, 100.0) as u16)
        .label(format!("{:.1}%  ({:.1} / {:.1} GiB)", mem_pct, mem_used as f64 / 1_073_741_824.0, mem_total as f64 / 1_073_741_824.0)), metrics[1]);

    let items: Vec<ListItem> = app.findings.iter().map(|s| ListItem::new(s.as_str())).collect();
    frame.render_widget(List::new(items).block(Block::default().borders(Borders::ALL).title(" Notes ")), rows[2]);
}

fn draw_diagnostics(frame: &mut Frame, app: &App, area: Rect) {
    let text = vec![
        Line::from(Span::styled("Diagnostic modules", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from("  [INFO] System inventory     Live CPU, memory and OS metadata are available."),
        Line::from("  [TODO] Windows integrity    DISM / SFC status checks are not implemented yet."),
        Line::from("  [TODO] Devices & drivers    Device problem codes are not implemented yet."),
        Line::from("  [TODO] Event logs           Event Viewer analysis is not implemented yet."),
        Line::from("  [TODO] Security             Defender and Firewall checks are not implemented yet."),
        Line::from("  [TODO] Network              Adapter, DNS and connectivity checks are not implemented yet."),
        Line::from(""),
        Line::from(format!("Last inventory refresh: {:.1}s ago", app.refreshed.elapsed().as_secs_f32())),
        Line::from("No scan has been run. TODO entries are planned work, not detected problems."),
    ];
    frame.render_widget(Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(" Diagnostics ")).wrap(Wrap { trim: false }), area);
}

fn draw_storage(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![Line::from(Span::styled("Detected volumes", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))), Line::from("")];
    if app.disks.is_empty() { lines.push(Line::from("No volumes reported by the system inventory API.")); }
    for disk in app.disks.list() {
        let total = disk.total_space();
        let avail = disk.available_space();
        let used = total.saturating_sub(avail);
        lines.push(Line::from(format!("{}  {:?}  {} / {} GiB used  [{}]",
            disk.mount_point().display(), disk.file_system(),
            used / 1_073_741_824, total / 1_073_741_824, disk.kind())));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Note: this is volume inventory only. SMART, filesystem integrity and physical-drive health are not checked yet."));
    frame.render_widget(Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" Storage ")).wrap(Wrap { trim: true }), area);
}

fn draw_about(frame: &mut Frame, area: Rect) {
    let text = vec![
        Line::from(Span::styled("SystemWatch 0.1.0 вЂ” early development", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from("A portable, read-only Windows diagnostic toolkit."),
        Line::from(""),
        Line::from("Safety: this build only reads basic OS, CPU, memory and volume inventory."),
        Line::from("It does not repair, delete, disable, or modify system settings."),
        Line::from(""),
        Line::from("Some checks may require administrator rights or may not exist on every Windows version."),
        Line::from("Unavailable checks must never be treated as a clean bill of health."),
        Line::from(""),
        Line::from("Project: github.com/Caracoto4/SystemWatch"),
    ];
    frame.render_widget(Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(" About ")).wrap(Wrap { trim: true }), area);
}