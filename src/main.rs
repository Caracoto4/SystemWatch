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
use std::{io, process::Command, time::{Duration, Instant}};
use sysinfo::{Disks, System};

#[derive(Clone)]
struct CheckResult {
    name: String,
    status: String,
    detail: String,
}

struct App {
    tab: usize,
    should_quit: bool,
    system: System,
    disks: Disks,
    refreshed: Instant,
    host: String,
    os: String,
    findings: Vec<String>,
    checks: Vec<CheckResult>,
    scan_completed: Option<Instant>,
    scan_running: bool,
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
            tab: 0,
            should_quit: false,
            system,
            disks,
            refreshed: Instant::now(),
            host,
            os,
            findings: vec![
                "No diagnostic scans have been run yet.".into(),
                "Live inventory is informational; it is not a full health verdict.".into(),
            ],
            checks: Vec::new(),
            scan_completed: None,
            scan_running: false,
        }
    }

    fn refresh(&mut self) {
        self.system.refresh_all();
        self.disks.refresh(true);
        self.refreshed = Instant::now();
    }

    fn run_scan(&mut self) {
        self.scan_running = true;
        self.checks = run_windows_checks();
        self.scan_completed = Some(Instant::now());
        self.scan_running = false;
    }

    fn cpu_usage(&self) -> f32 {
        let cpus = self.system.cpus();
        if cpus.is_empty() {
            0.0
        } else {
            cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / cpus.len() as f32
        }
    }
}

fn parse_check_line(name: &str, line: &str) -> CheckResult {
    let mut parts = line.splitn(3, '|');
    let status = parts.next().unwrap_or("UNAVAILABLE").trim().to_string();
    let detail = parts.next().unwrap_or("No status returned.").trim().to_string();
    let message = parts.next().unwrap_or("").trim();
    let valid = ["HEALTHY", "WARNING", "CRITICAL", "UNAVAILABLE"];
    if !valid.contains(&status.as_str()) {
        return CheckResult {
            name: name.to_string(),
            status: "UNAVAILABLE".into(),
            detail: "The check returned an unrecognized status format.".into(),
        };
    }
    CheckResult {
        name: name.to_string(),
        status,
        detail: if message.is_empty() { detail } else { format!("{detail} — {message}") },
    }
}

fn powershell_check(name: &str, script: &str) -> CheckResult {
    let output = Command::new("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", script])
        .output();

    match output {
        Ok(result) if result.status.success() => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let line = stdout.lines().find(|line| !line.trim().is_empty()).unwrap_or("");
            parse_check_line(name, line)
        }
        Ok(result) => CheckResult {
            name: name.to_string(),
            status: "UNAVAILABLE".into(),
            detail: String::from_utf8_lossy(&result.stderr).lines().next()
                .unwrap_or("PowerShell check failed.").trim().to_string(),
        },
        Err(error) => CheckResult {
            name: name.to_string(),
            status: "UNAVAILABLE".into(),
            detail: format!("Could not start PowerShell: {error}"),
        },
    }
}

fn run_windows_checks() -> Vec<CheckResult> {
    let devices = r#"$ErrorActionPreference='Stop'; try { $bad=@(Get-CimInstance -ClassName Win32_PnPEntity | Where-Object { $_.ConfigManagerErrorCode -ne $null -and $_.ConfigManagerErrorCode -ne 0 }); if($bad.Count -eq 0){ 'HEALTHY|No device problem codes reported|Device Manager inventory' } else { $items=($bad | Select-Object -First 4 | ForEach-Object { '{0} (code {1})' -f $_.Name,$_.ConfigManagerErrorCode }) -join '; '; 'WARNING|{0} device(s) report a problem code|{1}' -f $bad.Count,$items } } catch { 'UNAVAILABLE|Device inventory query failed|' + $_.Exception.Message }"#;
    let defender = r#"$ErrorActionPreference='Stop'; try { $s=Get-MpComputerStatus; if($s.RealTimeProtectionEnabled){ 'HEALTHY|Real-time protection is enabled|Microsoft Defender status' } else { 'WARNING|Real-time protection is reported disabled|Microsoft Defender status' } } catch { 'UNAVAILABLE|Defender status could not be read|' + $_.Exception.Message }"#;
    let firewall = r#"$ErrorActionPreference='Stop'; try { $p=@(Get-NetFirewallProfile); if($p.Count -eq 0){ 'UNAVAILABLE|No firewall profiles returned|' } else { $off=@($p | Where-Object { -not $_.Enabled }); if($off.Count -eq 0){ 'HEALTHY|All returned firewall profiles are enabled|Domain, Private and Public profiles' } else { $names=($off | ForEach-Object { $_.Name }) -join ', '; 'WARNING|{0} firewall profile(s) are disabled|{1}' -f $off.Count,$names } } } catch { 'UNAVAILABLE|Firewall status could not be read|' + $_.Exception.Message }"#;

    vec![
        powershell_check("Devices & drivers", devices),
        powershell_check("Microsoft Defender", defender),
        powershell_check("Windows Firewall", firewall),
    ]
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
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                    KeyCode::Right | KeyCode::Tab => app.tab = (app.tab + 1) % 4,
                    KeyCode::Left => app.tab = (app.tab + 3) % 4,
                    KeyCode::Char('1') => app.tab = 0,
                    KeyCode::Char('2') => app.tab = 1,
                    KeyCode::Char('3') => app.tab = 2,
                    KeyCode::Char('4') => app.tab = 3,
                    KeyCode::Char('r') => app.refresh(),
                    KeyCode::Char('s' | 'S') if app.tab == 1 => app.run_scan(),
                    _ => {}
                }
            }
        }
        if app.refreshed.elapsed() >= Duration::from_secs(2) {
            app.refresh();
        }
    }
    Ok(())
}

fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(8), Constraint::Length(2)])
        .split(area);
    let tabs = ["Overview", "Diagnostics", "Storage", "About"];
    let mut spans = vec![Span::styled(
        " SYSTEMWATCH ",
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )];
    for (i, tab) in tabs.iter().enumerate() {
        let style = if app.tab == i {
            Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        spans.push(Span::styled(format!("  {}:{}  ", i + 1, tab), style));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans))
            .block(Block::default().borders(Borders::ALL).title(" Portable Windows diagnostics ")),
        chunks[0],
    );
    match app.tab {
        0 => draw_overview(frame, app, chunks[1]),
        1 => draw_diagnostics(frame, app, chunks[1]),
        2 => draw_storage(frame, app, chunks[1]),
        _ => draw_about(frame, chunks[1]),
    }
    frame.render_widget(
        Paragraph::new("←/→ or 1-4: navigate   R: refresh   S: scan diagnostics   Q/Esc: quit   •   Read-only"),
        chunks[2],
    );
}

fn draw_overview(frame: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Length(7), Constraint::Min(5)])
        .split(area);
    let cpu = app.cpu_usage();
    let mem_total = app.system.total_memory();
    let mem_used = app.system.used_memory();
    let mem_pct = if mem_total == 0 { 0.0 } else { mem_used as f64 / mem_total as f64 * 100.0 };
    let summary = format!(
        "Host: {}\nOS: {}\nCPU: {} logical processors",
        app.host, app.os, app.system.cpus().len()
    );
    frame.render_widget(
        Paragraph::new(summary)
            .block(Block::default().borders(Borders::ALL).title(" System "))
            .wrap(Wrap { trim: true }),
        rows[0],
    );
    let metrics = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);
    frame.render_widget(
        Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(" CPU average "))
            .gauge_style(Style::default().fg(Color::Cyan))
            .percent(cpu.clamp(0.0, 100.0) as u16)
            .label(format!("{:.1}%", cpu)),
        metrics[0],
    );
    frame.render_widget(
        Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(" Memory "))
            .gauge_style(Style::default().fg(Color::Yellow))
            .percent(mem_pct.clamp(0.0, 100.0) as u16)
            .label(format!(
                "{:.1}%  ({:.1} / {:.1} GiB)",
                mem_pct,
                mem_used as f64 / 1_073_741_824.0,
                mem_total as f64 / 1_073_741_824.0
            )),
        metrics[1],
    );
    let items: Vec<ListItem> = app.findings.iter().map(|s| ListItem::new(s.as_str())).collect();
    frame.render_widget(
        List::new(items).block(Block::default().borders(Borders::ALL).title(" Notes ")),
        rows[2],
    );
}

fn status_color(status: &str) -> Color {
    match status {
        "HEALTHY" => Color::Green,
        "WARNING" => Color::Yellow,
        "CRITICAL" => Color::Red,
        _ => Color::DarkGray,
    }
}

fn draw_diagnostics(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![
        Line::from(Span::styled(
            "Read-only Windows checks",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from("Press S to run the checks. They may take a few seconds."),
        Line::from(""),
    ];
    if app.checks.is_empty() {
        lines.push(Line::from("No diagnostic scan has been run in this session."));
        lines.push(Line::from(""));
        lines.push(Line::from("Checks available in this preview:"));
        lines.push(Line::from("  • Plug and Play device problem codes"));
        lines.push(Line::from("  • Microsoft Defender real-time protection status"));
        lines.push(Line::from("  • Windows Firewall profile status"));
    } else {
        for check in &app.checks {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("[{}] ", check.status),
                    Style::default().fg(status_color(&check.status)).add_modifier(Modifier::BOLD),
                ),
                Span::styled(check.name.clone(), Style::default().add_modifier(Modifier::BOLD)),
            ]));
            lines.push(Line::from(format!("  {}", check.detail)));
            lines.push(Line::from(""));
        }
        if let Some(done) = app.scan_completed {
            lines.push(Line::from(format!("Scan completed {:.0}s ago.", done.elapsed().as_secs_f32())));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Unavailable means the check could not produce a result; it does not mean healthy."));
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(" Diagnostics "))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_storage(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![
        Line::from(Span::styled(
            "Detected volumes",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    if app.disks.is_empty() {
        lines.push(Line::from("No volumes reported by the system inventory API."));
    }
    for disk in app.disks.list() {
        let total = disk.total_space();
        let avail = disk.available_space();
        let used = total.saturating_sub(avail);
        let pct = if total == 0 { 0.0 } else { used as f64 / total as f64 * 100.0 };
        lines.push(Line::from(format!(
            "{}  {:?}  {:.1}% used ({:.1} / {:.1} GiB)  [{}]",
            disk.mount_point().display(),
            disk.file_system(),
            pct,
            used as f64 / 1_073_741_824.0,
            total as f64 / 1_073_741_824.0,
            disk.kind()
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(
        "Volume inventory only. SMART, filesystem integrity and physical-drive health are not checked yet.",
    ));
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(" Storage "))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_about(frame: &mut Frame, area: Rect) {
    let text = vec![
        Line::from(Span::styled(
            "SystemWatch 0.1.0 — early development",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("A portable, read-only Windows diagnostic toolkit."),
        Line::from(""),
        Line::from("This preview reads basic OS, CPU, memory, volume and selected security/device status."),
        Line::from("It does not repair, delete, disable, or modify system settings."),
        Line::from(""),
        Line::from("Some checks may require administrator rights or may not exist on every Windows version."),
        Line::from("Unavailable checks are never treated as a clean bill of health."),
        Line::from(""),
        Line::from("Project: github.com/Caracoto4/SystemWatch"),
    ];
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(" About "))
            .wrap(Wrap { trim: true }),
        area,
    );
}
#[cfg(test)]
mod tests {
    use super::parse_check_line;

    #[test]
    fn parses_healthy_result() {
        let result = parse_check_line("Devices", "HEALTHY|No problems|Device inventory");
        assert_eq!(result.status, "HEALTHY");
        assert!(result.detail.contains("No problems"));
        assert!(result.detail.contains("Device inventory"));
    }

    #[test]
    fn parses_warning_result() {
        let result = parse_check_line("Firewall", "WARNING|A profile is disabled|Public");
        assert_eq!(result.status, "WARNING");
        assert!(result.detail.contains("Public"));
    }

    #[test]
    fn malformed_status_becomes_unavailable() {
        let result = parse_check_line("Example", "MAYBE|Unknown");
        assert_eq!(result.status, "UNAVAILABLE");
    }

    #[test]
    fn missing_output_becomes_unavailable() {
        let result = parse_check_line("Example", "");
        assert_eq!(result.status, "UNAVAILABLE");
    }
}