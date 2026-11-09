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
use std::{fs, io, path::PathBuf, process::Command, sync::mpsc::{self, Receiver, TryRecvError}, thread, time::{Duration, Instant}};
use sysinfo::{Disks, System};

#[derive(Clone)]
struct CheckResult {
    name: String,
    status: String,
    detail: String,
    evidence: Vec<String>,
    duration_ms: u64,
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
    scan_receiver: Option<Receiver<Vec<CheckResult>>>,
    scan_started_at: Option<chrono::DateTime<chrono::Local>>,
    scan_log_path: Option<PathBuf>,
    scan_log_error: Option<String>,
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
            scan_receiver: None,
            scan_started_at: None,
            scan_log_path: None,
            scan_log_error: None,
        }
    }

    fn refresh(&mut self) {
        self.system.refresh_all();
        self.disks.refresh(true);
        self.refreshed = Instant::now();
    }

    fn start_scan(&mut self) {
        if self.scan_running {
            return;
        }
        self.scan_started_at = Some(chrono::Local::now());
        self.scan_log_path = None;
        self.scan_log_error = None;
        let (sender, receiver) = mpsc::channel();
        self.scan_receiver = Some(receiver);
        self.scan_running = true;
        let spawn_result = thread::Builder::new()
            .name("systemwatch-diagnostics".into())
            .spawn(move || {
                let results = run_windows_checks();
                let _ = sender.send(results);
            });
        if let Err(error) = spawn_result {
            self.scan_running = false;
            self.scan_receiver = None;
            self.checks = vec![CheckResult {
                name: "Diagnostics worker".into(),
                status: "UNAVAILABLE".into(),
                detail: format!("Could not start background scan: {error}"),
                evidence: Vec::new(),
                duration_ms: 0,
            }];
        }
    }

    fn poll_scan(&mut self) {
        let received = match self.scan_receiver.as_ref() {
            Some(receiver) => match receiver.try_recv() {
                Ok(results) => Some(Some(results)),
                Err(TryRecvError::Disconnected) => Some(None),
                Err(TryRecvError::Empty) => None,
            },
            None => None,
        };
        match received {
            Some(Some(results)) => {
                let finished_at = chrono::Local::now();
                if let Some(started_at) = self.scan_started_at.take() {
                    match write_scan_log(&self.host, &self.os, started_at, finished_at, &results) {
                        Ok(path) => self.scan_log_path = Some(path),
                        Err(error) => self.scan_log_error = Some(format!("{error:#}")),
                    }
                }
                self.checks = results;
                self.scan_completed = Some(Instant::now());
                self.scan_running = false;
                self.scan_receiver = None;
            }
            Some(None) => {
                self.checks = vec![CheckResult {
                    name: "Diagnostics worker".into(),
                    status: "UNAVAILABLE".into(),
                    detail: "The background scan ended without returning results.".into(),
                    evidence: Vec::new(),
                    duration_ms: 0,
                }];
                self.scan_running = false;
                self.scan_receiver = None;
            }
            None => {}
        }
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
            evidence: Vec::new(),
            duration_ms: 0,
        };
    }
    CheckResult {
        name: name.to_string(),
        status,
        detail: if message.is_empty() { detail } else { format!("{detail} — {message}") },
        evidence: Vec::new(),
        duration_ms: 0,
    }
}

fn powershell_check(name: &str, script: &str) -> CheckResult {
    let started = Instant::now();
    let utf8_script = format!("[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding; $OutputEncoding = [Console]::OutputEncoding; {script}");
    let output = Command::new("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", &utf8_script])
        .output();

    let mut check = match output {
        Ok(result) if result.status.success() => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let mut lines = stdout.lines().filter(|line| !line.trim().is_empty());
            let line = lines.next().unwrap_or("");
            let mut check = parse_check_line(name, line);
            check.evidence = lines.map(str::trim).filter(|line| !line.is_empty()).map(str::to_string).collect();
            check
        }
        Ok(result) => CheckResult {
            name: name.to_string(),
            status: "UNAVAILABLE".into(),
            detail: {
                let message = String::from_utf8_lossy(&result.stderr).trim().to_string();
                if message.is_empty() { "PowerShell check failed without an error message.".to_string() } else { message }
            },
            evidence: Vec::new(),
            duration_ms: 0,
        },
        Err(error) => CheckResult {
            name: name.to_string(),
            status: "UNAVAILABLE".into(),
            detail: format!("Could not start PowerShell: {error}"),
            evidence: Vec::new(),
            duration_ms: 0,
        },
    };
    check.duration_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    check
}
fn run_windows_checks() -> Vec<CheckResult> {
    let devices = r#"$ErrorActionPreference='Stop'; try { $bad=@(Get-CimInstance -ClassName Win32_PnPEntity | Where-Object { $_.ConfigManagerErrorCode -ne $null -and $_.ConfigManagerErrorCode -ne 0 }); if($bad.Count -eq 0){ 'HEALTHY|No device problem codes reported|Device Manager inventory' } else { 'WARNING|{0} device(s) report a problem code|See detailed findings' -f $bad.Count }; ('PnP devices enumerated: {0}' -f (@(Get-CimInstance Win32_PnPEntity)).Count); foreach($d in ($bad | Select-Object -First 100)){ 'Problem device: {0}; code={1}; status={2}; class={3}' -f $d.Name,$d.ConfigManagerErrorCode,$d.Status,$d.PNPClass } } catch { 'UNAVAILABLE|Device inventory query failed|' + $_.Exception.Message }"#;
    let defender = r#"$ErrorActionPreference='Stop'; try { $s=Get-MpComputerStatus; if($s.RealTimeProtectionEnabled){ 'HEALTHY|Real-time protection is enabled|Microsoft Defender status' } else { 'WARNING|Real-time protection is reported disabled|Microsoft Defender status' }; foreach($k in @('AMServiceEnabled','AntivirusEnabled','AntispywareEnabled','BehaviorMonitorEnabled','IoavProtectionEnabled','NISEnabled','RealTimeProtectionEnabled','OnAccessProtectionEnabled','QuickScanAge','FullScanAge','AntivirusSignatureVersion','AntivirusSignatureLastUpdated','AMEngineVersion','ComputerState')){ $v=$s.$k; if($null -ne $v){ '{0}: {1}' -f $k,$v } } } catch { 'UNAVAILABLE|Defender status could not be read|' + $_.Exception.Message }"#;
    let firewall = r#"$ErrorActionPreference='Stop'; try { $p=@(Get-NetFirewallProfile); if($p.Count -eq 0){ 'UNAVAILABLE|No firewall profiles returned|' } else { $off=@($p | Where-Object { -not $_.Enabled }); if($off.Count -eq 0){ 'HEALTHY|All returned firewall profiles are enabled|Get-NetFirewallProfile' } else { 'WARNING|{0} firewall profile(s) are disabled|See detailed findings' -f $off.Count }; foreach($x in $p){ 'Profile: {0}; enabled={1}; inbound={2}; outbound={3}; notifications={4}; policy={5}' -f $x.Name,$x.Enabled,$x.DefaultInboundAction,$x.DefaultOutboundAction,$x.NotifyOnListen,$x.PolicyStoreSource } } } catch { 'UNAVAILABLE|Firewall status could not be read|' + $_.Exception.Message }"#;

    let services = r#"$ErrorActionPreference='Stop'; try { $stopped=@(Get-CimInstance -ClassName Win32_Service | Where-Object { $_.StartMode -eq 'Auto' -and $_.State -eq 'Stopped' -and $_.ExitCode -ne 0 }); if($stopped.Count -eq 0){ 'HEALTHY|No stopped automatic service with a non-zero exit code was found|Conservative service check' } else { 'WARNING|{0} stopped automatic service(s) report a non-zero exit code|See detailed findings' -f $stopped.Count }; ('Services enumerated: {0}' -f (@(Get-CimInstance Win32_Service)).Count); foreach($x in ($stopped | Select-Object -First 100)){ 'Service: {0}; display={1}; state={2}; start={3}; exit={4}; account={5}' -f $x.Name,$x.DisplayName,$x.State,$x.StartMode,$x.ExitCode,$x.StartName } } catch { 'UNAVAILABLE|Windows service inventory could not be queried|' + $_.Exception.Message }"#;
    let event_log = r#"$ErrorActionPreference='Stop'; try { $since=(Get-Date).AddHours(-24); $all=@(); foreach($log in @('System','Application')) { try { $items=@(Get-WinEvent -FilterHashtable @{LogName=$log; Level=1,2; StartTime=$since} -MaxEvents 501 -ErrorAction Stop); $all += $items | ForEach-Object { [pscustomobject]@{ Log=$log; Level=$_.Level } } } catch { if ($_.FullyQualifiedErrorId -notlike 'NoMatchingEventsFound*') { throw } } }; $count=$all.Count; $sys=@($all | Where-Object {$_.Log -eq 'System'}).Count; $app=@($all | Where-Object {$_.Log -eq 'Application'}).Count; if($count -eq 0){ 'HEALTHY|No critical or error events found in the queried 24-hour window|System and Application logs' } else { 'WARNING|{0} critical/error event(s) in the last 24 hours|System: {1}; Application: {2}' -f $count,$sys,$app }; foreach($log in @('System','Application')){ try { foreach($e in @(Get-WinEvent -FilterHashtable @{LogName=$log;Level=1,2;StartTime=$since} -MaxEvents 50 -ErrorAction Stop)){ 'Event: log={0}; time={1}; level={2}; id={3}; provider={4}; record={5}' -f $log,$e.TimeCreated,$e.LevelDisplayName,$e.Id,$e.ProviderName,$e.RecordId } } catch { if($_.FullyQualifiedErrorId -notlike 'NoMatchingEventsFound*'){ 'Event sample unavailable for {0}: {1}' -f $log,$_.Exception.Message } } } } catch { 'UNAVAILABLE|Windows event logs could not be queried|' + $_.Exception.Message }"#;
    let disk_health = r#"$ErrorActionPreference='Stop'; try { $disks=@(Get-PhysicalDisk -ErrorAction Stop); if($disks.Count -eq 0){ 'UNAVAILABLE|No physical disks were returned by the Storage provider|' } else { $bad=@($disks | Where-Object { $_.HealthStatus -eq 'Unhealthy' }); $warn=@($disks | Where-Object { $_.HealthStatus -eq 'Warning' }); $unknown=@($disks | Where-Object { $_.HealthStatus -notin @('Healthy','Warning','Unhealthy') }); $items=@(); foreach($d in ($disks | Select-Object -First 100)){ $label=('{0}: health={1}; operational={2}' -f $d.FriendlyName,$d.HealthStatus,($d.OperationalStatus -join ',')); try { $r=Get-StorageReliabilityCounter -PhysicalDisk $d -ErrorAction Stop; $extra=@(); if($null -ne $r.Temperature -and $r.Temperature -gt 0){ $extra += ('temperature={0}C' -f $r.Temperature) }; if($null -ne $r.Wear -and $r.Wear -ge 0){ $extra += ('wear={0}%' -f $r.Wear) }; if($extra.Count -gt 0){ $label += '; telemetry: ' + ($extra -join ', ') } } catch { }; $items += $label }; if($bad.Count -gt 0){ 'CRITICAL|{0} disk(s) report Unhealthy|{1}' -f $bad.Count,($items -join '; ') } elseif($warn.Count -gt 0){ 'WARNING|{0} disk(s) report Warning health status|{1}' -f $warn.Count,($items -join '; ') } elseif($unknown.Count -eq $disks.Count){ 'UNAVAILABLE|Physical disks were found but health status is not reported|{0}' -f ($items -join '; ') } elseif($unknown.Count -gt 0){ 'WARNING|Health status is unknown for {0} of {1} disk(s)|{2}' -f $unknown.Count,$disks.Count,($items -join '; ') } else { 'HEALTHY|All returned physical disks report Healthy|{0}; reliability telemetry is shown only when exposed by the device' -f ($items -join '; ') } }; foreach($item in $items){ 'Disk detail: ' + $item } } catch { 'UNAVAILABLE|Physical disk health could not be queried|' + $_.Exception.Message }"#;
    vec![
        powershell_check("Devices & drivers", devices),
        powershell_check("Microsoft Defender", defender),
        powershell_check("Windows Firewall", firewall),
        powershell_check("Windows Services", services),
        powershell_check("Windows Event Log (24h)", event_log),
        powershell_check("Physical disk health", disk_health),
    ]
}



fn collect_system_info() -> Vec<String> {
    let script = r#"$ErrorActionPreference='Stop'; try {
      $os=Get-CimInstance Win32_OperatingSystem
      $cs=Get-CimInstance Win32_ComputerSystem
      $cpu=@(Get-CimInstance Win32_Processor)
      $bios=Get-CimInstance Win32_BIOS
      $board=Get-CimInstance Win32_BaseBoard
      $gpu=@(Get-CimInstance Win32_VideoController)
      $ram=@(Get-CimInstance Win32_PhysicalMemory)
      'Manufacturer / model: {0} / {1}' -f $cs.Manufacturer,$cs.Model
      'OS edition: {0}; version: {1}; build: {2}; architecture: {3}' -f $os.Caption,$os.Version,$os.BuildNumber,$os.OSArchitecture
      'Install date: {0}; last boot: {1}' -f $os.InstallDate,$os.LastBootUpTime
      'System type: {0}; domain/workgroup: {1}' -f $cs.SystemType,$cs.Domain
      'Current user: {0}' -f [Security.Principal.WindowsIdentity]::GetCurrent().Name
      'CPU packages: {0}; physical cores: {1}; logical processors: {2}' -f $cpu.Count,(($cpu|Measure-Object NumberOfCores -Sum).Sum),(($cpu|Measure-Object NumberOfLogicalProcessors -Sum).Sum)
      foreach($c in $cpu){ 'CPU: {0}; cores={1}; threads={2}; max MHz={3}' -f $c.Name,$c.NumberOfCores,$c.NumberOfLogicalProcessors,$c.MaxClockSpeed }
      'Installed RAM: {0:N2} GB; modules: {1}' -f ($cs.TotalPhysicalMemory/1GB),$ram.Count
      foreach($m in $ram){ 'RAM module: {0:N0} MB; speed={1} MHz; manufacturer={2}; part={3}' -f ($m.Capacity/1MB),$m.Speed,$m.Manufacturer,$m.PartNumber }
      'BIOS: {0}; version: {1}; date: {2}' -f $bios.Manufacturer,($bios.SMBIOSBIOSVersion -join ', '),$bios.ReleaseDate
      'Motherboard: {0} {1}' -f $board.Manufacturer,$board.Product
      foreach($g in $gpu){ 'GPU: {0}; driver={1}; video memory={2}' -f $g.Name,$g.DriverVersion,$g.AdapterRAM }
      foreach($d in @(Get-CimInstance Win32_LogicalDisk -Filter 'DriveType=3')){ 'Volume {0} [{1}]: {2:N1} GB free / {3:N1} GB total; filesystem={4}' -f $d.DeviceID,$d.VolumeName,($d.FreeSpace/1GB),($d.Size/1GB),$d.FileSystem }
      foreach($n in @(Get-NetAdapter -ErrorAction SilentlyContinue|Where-Object {$_.HardwareInterface})){ 'Network adapter: {0}; status={1}; speed={2}; interface={3}' -f $n.Name,$n.Status,$n.LinkSpeed,$n.InterfaceDescription }
    } catch { 'System inventory unavailable: ' + $_.Exception.Message }"#;
    let utf8_script = format!("[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding; $OutputEncoding = [Console]::OutputEncoding; {script}");
    match Command::new("powershell.exe").args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", &utf8_script]).output() {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout).lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_string).collect(),
        Ok(output) => vec![format!("System inventory query failed: {}", String::from_utf8_lossy(&output.stderr).trim())],
        Err(error) => vec![format!("Could not start PowerShell for system inventory: {error}")],
    }
}

fn write_scan_log(
    host: &str,
    os: &str,
    started_at: chrono::DateTime<chrono::Local>,
    finished_at: chrono::DateTime<chrono::Local>,
    checks: &[CheckResult],
) -> Result<PathBuf> {
    let executable = std::env::current_exe().context("Could not locate the running executable")?;
    let directory = executable.parent().context("Executable path has no parent directory")?;
    let filename = format!(
        "SystemWatch_scan_{}_pid{}.log",
        finished_at.format("%Y%m%d_%H%M%S_%3f"),
        std::process::id()
    );
    let path = directory.join(filename);
    let duration = finished_at.signed_duration_since(started_at);
    let healthy = checks.iter().filter(|check| check.status == "HEALTHY").count();
    let warning = checks.iter().filter(|check| check.status == "WARNING").count();
    let critical = checks.iter().filter(|check| check.status == "CRITICAL").count();
    let unavailable = checks.iter().filter(|check| check.status == "UNAVAILABLE").count();
    let mut report = String::new();
    use std::fmt::Write as FmtWrite;
    writeln!(&mut report, "SYSTEMWATCH DIAGNOSTIC SCAN")?;
    writeln!(&mut report, "========================================")?;
    writeln!(&mut report, "Application: SystemWatch 0.1.0")?;
    writeln!(&mut report, "Host: {host}")?;
    writeln!(&mut report, "Operating system: {os}")?;
    writeln!(&mut report, "")?;
    writeln!(&mut report, "SYSTEM INFORMATION")?;
    writeln!(&mut report, "----------------------------------------")?;
    for item in collect_system_info() {
        writeln!(&mut report, "{item}")?;
    }
    writeln!(&mut report, "Process ID: {}", std::process::id())?;
    writeln!(&mut report, "Started: {}", started_at.format("%Y-%m-%d %H:%M:%S%.3f %:z"))?;
    writeln!(&mut report, "Finished: {}", finished_at.format("%Y-%m-%d %H:%M:%S%.3f %:z"))?;
    writeln!(&mut report, "Duration: {} ms", duration.num_milliseconds())?;
    writeln!(&mut report, "Checks completed: {}", checks.len())?;
    let overall = if critical > 0 { "ATTENTION REQUIRED" } else if warning > 0 { "REVIEW RECOMMENDED" } else if unavailable > 0 { "PARTIAL RESULTS" } else { "NO ISSUES DETECTED BY AVAILABLE CHECKS" };
    writeln!(&mut report, "Overall assessment: {overall}")?;
    writeln!(&mut report, "Summary: {healthy} healthy, {warning} warning, {critical} critical, {unavailable} unavailable")?;
    let attention: Vec<&CheckResult> = checks.iter().filter(|check| check.status != "HEALTHY").collect();
    if attention.is_empty() {
        writeln!(&mut report, "Items requiring attention: none reported by these checks")?;
    } else {
        writeln!(&mut report, "Items requiring attention:")?;
        for check in attention {
            writeln!(&mut report, "  - [{}] {}: {}", check.status, check.name, check.detail)?;
        }
    }
    writeln!(&mut report, "")?;
    for (index, check) in checks.iter().enumerate() {
        let source = match check.name.as_str() {
            "Devices & drivers" => "CIM Win32_PnPEntity / ConfigManagerErrorCode",
            "Microsoft Defender" => "Get-MpComputerStatus / RealTimeProtectionEnabled",
            "Windows Firewall" => "Get-NetFirewallProfile / Enabled",
            "Windows Services" => "CIM Win32_Service / StartMode, State, ExitCode",
            "Windows Event Log (24h)" => "Get-WinEvent / System + Application / Critical + Error / last 24h",
            "Physical disk health" => "Get-PhysicalDisk / HealthStatus + Get-StorageReliabilityCounter when supported",
            _ => "Internal diagnostic result",
        };
        writeln!(&mut report, "{}. [{}] {}", index + 1, check.status, check.name)?;
        writeln!(&mut report, "   Source: {source}")?;
        writeln!(&mut report, "   Result: {}", check.detail)?;
        writeln!(&mut report, "   Duration: {} ms", check.duration_ms)?;
        if !check.evidence.is_empty() {
            writeln!(&mut report, "   Detailed findings:")?;
            for item in &check.evidence {
                writeln!(&mut report, "      - {item}")?;
            }
        }
        writeln!(&mut report, "")?;
    }
    writeln!(&mut report, "========================================")?;
    writeln!(&mut report, "SystemWatch is read-only. A status applies only to the named check.")?;
    writeln!(&mut report, "Event log entries are summarized; raw event message text is not included.")?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .with_context(|| format!("Could not create scan log {} without overwriting an existing file", path.display()))?;
    std::io::Write::write_all(&mut file, report.as_bytes())
        .with_context(|| format!("Could not write scan log {}", path.display()))?;
    Ok(path)
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
                    KeyCode::Char('s' | 'S') if app.tab == 1 => app.start_scan(),
                    _ => {}
                }
            }
        }
        app.poll_scan();
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
        Line::from(if app.scan_running { "Scan is running in the background; the interface remains responsive." } else { "Press S to run the checks. They run in the background." }),
        Line::from(""),
    ];
    if app.checks.is_empty() {
        lines.push(Line::from(if app.scan_running { "Diagnostics are running..." } else { "No diagnostic scan has been run in this session." }));
        lines.push(Line::from(""));
        lines.push(Line::from("Checks available in this preview:"));
        lines.push(Line::from("  • Plug and Play device problem codes"));
        lines.push(Line::from("  • Microsoft Defender real-time protection status"));
        lines.push(Line::from("  • Windows Firewall profile status"));
        lines.push(Line::from("  • Windows services with non-zero failure codes"));
        lines.push(Line::from("  • System and Application critical/error event counts (24h)"));
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
            lines.push(Line::from(format!("  Duration: {} ms", check.duration_ms)));
            lines.push(Line::from(""));
        }
        if let Some(done) = app.scan_completed {
            lines.push(Line::from(format!("Scan completed {:.0}s ago.", done.elapsed().as_secs_f32())));
        }
        if let Some(path) = &app.scan_log_path {
            lines.push(Line::from(format!("Detailed log saved: {}", path.display())));
        }
        if let Some(error) = &app.scan_log_error {
            lines.push(Line::from(format!("Could not save scan log: {error}")));
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
        Line::from("This preview reads basic OS, CPU, memory, volume, security/device status and selected event-log counters."),
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
    use super::{parse_check_line, run_windows_checks, write_scan_log, CheckResult};
    use std::fs;

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
    fn writes_detailed_scan_log_next_to_executable() {
        let started = chrono::Local::now() - chrono::Duration::seconds(1);
        let finished = chrono::Local::now();
        let checks = vec![CheckResult {
            name: "Example check".into(),
            status: "HEALTHY".into(),
            detail: "Test result".into(),
            evidence: vec!["Additional measured detail".into()],
            duration_ms: 42,
        }];
        let path = write_scan_log("TEST-HOST", "TEST-WINDOWS", started, finished, &checks)
            .expect("scan log should be written");
        let contents = fs::read_to_string(&path).expect("scan log should be readable");
        assert!(contents.contains("TEST-HOST"));
        assert!(contents.contains("TEST-WINDOWS"));
        assert!(contents.contains("Example check"));
        assert!(contents.contains("Test result"));
        assert!(contents.contains("SYSTEM INFORMATION"));
        assert!(contents.contains("Additional measured detail"));
        assert!(contents.contains("Source: Internal diagnostic result"));
        assert!(contents.contains("Detailed findings:"));
        assert!(contents.contains("Overall assessment:"));
        assert!(contents.contains("Items requiring attention:"));
        assert!(contents.contains("Duration: 42 ms"));
        assert!(write_scan_log("TEST-HOST", "TEST-WINDOWS", started, finished, &checks).is_err());
        fs::remove_file(path).expect("test scan log should be removed");
    }

    #[test]
    fn windows_diagnostic_checks_return_structured_results() {
        let results = run_windows_checks();
        assert_eq!(results.len(), 6);
        for check in &results {
            assert!(["HEALTHY", "WARNING", "CRITICAL", "UNAVAILABLE"].contains(&check.status.as_str()));
            assert!(!check.name.is_empty());
            assert!(!check.detail.is_empty());
            println!("[{}] {}: {} ({} detail lines)", check.status, check.name, check.detail, check.evidence.len());
        }
        let started = chrono::Local::now() - chrono::Duration::seconds(1);
        let finished = chrono::Local::now();
        let path = write_scan_log("LIVE-TEST", "Windows test", started, finished, &results)
            .expect("live scan report should be written");
        let report = fs::read_to_string(&path).expect("live scan report should be readable");
        assert!(report.contains("SYSTEM INFORMATION"));
        assert!(report.contains("Devices & drivers"));
        assert!(report.contains("Microsoft Defender"));
        assert!(report.contains("Detailed findings:"));
        println!("Live diagnostic report: {}", path.display());
        fs::remove_file(path).expect("live scan report should be removed");
    }

    #[test]
    fn missing_output_becomes_unavailable() {
        let result = parse_check_line("Example", "");
        assert_eq!(result.status, "UNAVAILABLE");
    }
}