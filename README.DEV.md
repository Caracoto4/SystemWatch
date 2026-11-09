# SystemWatch - Developer Notes

This file is the project's engineering memory. Update it whenever implementation, architecture, verification results, or project workflow changes. Keep README.md user-facing and this file technical.

## Project and environment

- Local working directory: C:\projects\SystemWatch
- GitHub: https://github.com/Caracoto4/SystemWatch
- Primary branch: main
- Rust 2021, Ratatui, Crossterm, sysinfo, chrono, anyhow.
- Windows diagnostics use read-only PowerShell/CIM/Storage commands.
- Current source is primarily in src/main.rs.

## Non-negotiable principles

- Read-only diagnostics; never perform automatic repairs.
- A failed or unsupported query is UNAVAILABLE, not HEALTHY.
- Do not infer a fault from an unfamiliar process name or a single metric.
- Reports should distinguish measured facts, warnings, unavailable data and interpretation.
- Protect privacy: logs are local and may contain account, machine, hardware, device, service and event metadata.

## Current diagnostic checks

1. Devices and drivers: Win32_PnPEntity and ConfigManagerErrorCode; report affected device details.
2. Microsoft Defender: Get-MpComputerStatus; real-time protection and available protection/scan/signature fields.
3. Windows Firewall: Get-NetFirewallProfile; enabled state and inbound/outbound defaults for each profile.
4. Windows Services: conservative warning condition for stopped automatic services with non-zero exit codes; report details.
5. Windows Event Log: System and Application Critical/Error events in the last 24 hours; bounded samples include time, level, event ID, provider and record ID. Raw event message text is intentionally excluded.
6. Physical disk health: Get-PhysicalDisk; HealthStatus, OperationalStatus, optional Get-StorageReliabilityCounter telemetry.

## Scan log format

- A unique SystemWatch_scan_YYYYMMDD_HHMMSS_mmm_pidNNNN.log is written beside the running executable.
- File creation uses create_new; an existing report is never overwritten.
- Header includes app version, host, OS, process ID, start/end timestamps, duration and status counts.
- The system inventory appears at the beginning of the report: machine model, OS details, CPU, RAM/modules, BIOS, motherboard, GPU/driver, fixed volumes and hardware network adapters.
- Each check includes its data source, status, summary and additional evidence lines returned by its PowerShell script.
- Event message bodies are not exported. Logs can still contain identifying system details; warn users before sharing.

## Development workflow - MUST FOLLOW

At the start of each session:
1. Inspect git status --short --branch, recent commits and remotes.
2. Read this file and the relevant source before editing.
3. Preserve existing user changes; never reset or overwrite unrelated work.

After each meaningful implementation step:
1. Run cargo test and cargo check; run cargo build --release before preparing a release executable.
2. Run git diff --check and inspect the complete diff.
3. Update both README.md and README.DEV.md.
4. Commit the verified changes to main.
5. Push main to origin (https://github.com/Caracoto4/SystemWatch) and verify the local branch is synchronized with origin/main.
6. Report exactly what was tested and whether the push succeeded. Never claim a push without checking it.

Never forget the GitHub synchronization step. If authentication/network prevents a push, leave the local commit intact and explicitly report that it remains unpushed.

## Current work: expanded scan reports

- Added CheckResult.evidence to preserve all non-empty PowerShell output lines after the first status line.
- Added system inventory collection to the top of each scan log.
- Expanded detailed PowerShell output for devices, Defender, firewall, services, event logs and physical disks.
- Added a Windows diagnostic smoke test that executes the six live checks and validates their result structure.
- PowerShell failures now preserve the full error text to make script errors diagnosable.
- Verified that the physical disk script emits its status line before evidence lines.
- Live smoke test on the development PC: all six PowerShell checks returned parseable statuses; system inventory and the full report were generated and inspected by assertions. The current machine reported 9 Critical/Error event entries in the sampled 24-hour window; this is a count, not a diagnosis.
- Verification: cargo test (6 passed), cargo check, cargo build --release and git diff --check succeeded.

## Known limitations / next work

- Some Windows providers expose incomplete hardware/SMART telemetry; unavailable data must remain explicit.
- Event-log samples are bounded to avoid unbounded report sizes.
- No full SMART attribute interpretation, WHEA analysis, filesystem integrity scan, network diagnostics or JSON/HTML export yet.
- rustfmt may not be installed on the development toolchain; check before using cargo fmt --check.
- Validate the new report on the actual PC by running a scan in the TUI and reviewing the generated log.