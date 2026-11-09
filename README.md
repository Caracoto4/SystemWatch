# SystemWatch

<p align="center">
  <strong>Portable Windows diagnostics. Clear results. No automatic repairs.</strong>
</p>

<p align="center">
  <a href="README.md"><img alt="English" src="https://img.shields.io/badge/English-1F6FEB?style=for-the-badge"></a>
  <a href="README.ru.md"><img alt="Русский" src="https://img.shields.io/badge/Русский-555555?style=for-the-badge"></a>
</p>

<p align="center">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows-0078D4?style=flat-square&logo=windows">
  <img alt="Language" src="https://img.shields.io/badge/language-Rust-orange?style=flat-square&logo=rust">
  <img alt="Interface" src="https://img.shields.io/badge/interface-terminal-20232A?style=flat-square">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-green?style=flat-square">
  <img alt="Status" src="https://img.shields.io/badge/status-early%20preview-yellow?style=flat-square">
</p>

SystemWatch is a read-only Windows diagnostic tool with an interactive terminal interface. It gathers useful system information, checks selected Windows components, and creates a detailed local report — without automatically changing your PC.

<p align="center">
  <img src="preview.png" alt="SystemWatch terminal interface" width="850">
</p>

## Highlights

- **System overview** — Windows version and kernel, CPU model and cores, memory usage, uptime, architecture, and storage capacity.
- **Device diagnostics** — detects Plug and Play devices reporting Device Manager problem codes.
- **Microsoft Defender** — displays real-time protection status.
- **Windows Firewall** — checks the enabled state of available firewall profiles.
- **Windows Services** — flags a conservative set of stopped automatic services with non-zero exit codes.
- **Event Log** — counts Critical and Error events in the System and Application logs over the last 24 hours.
- **Physical disk health** — reads Windows storage health and operational status, plus telemetry when the device exposes it.
- **Detailed scan reports** — saves a timestamped log beside the executable, including system inventory, findings, status totals, and check timings.
- **Responsive terminal UI** — navigate sections, refresh inventory, and run diagnostics while the interface remains responsive.

## Safety and privacy

**SystemWatch is read-only by design.** It does not automatically repair Windows, change security settings, delete files, or stop services. A failed or unsupported query is reported as `UNAVAILABLE`, never silently treated as healthy.

Scan logs stay on your computer. They can contain your computer/account name, hardware identifiers, device and service names, and event metadata. Review a report before sharing it.

## Get started

A packaged release is not available yet. You can build the project from source:

```powershell
git clone https://github.com/Caracoto4/SystemWatch.git
cd SystemWatch
cargo run
```

**Requirements:** Windows, Rust toolchain, and Cargo. Supported Windows versions are still being validated.

## Keyboard shortcuts

| Key | Action |
|:---:|---|
| `1`–`4` | Open a section |
| `←` / `→` or `Tab` | Switch sections |
| `R` | Refresh system inventory |
| `S` | Run diagnostics in the Diagnostics section |
| `Q` or `Esc` | Quit |

## Diagnostic result states

| Status | Meaning |
|---|---|
| `HEALTHY` | The check completed and did not report an issue within its scope. |
| `WARNING` | The check found something worth reviewing. |
| `CRITICAL` | The check reported a critical condition. |
| `UNAVAILABLE` | The check could not obtain reliable data. |

A clean result is not a guarantee that the entire PC is healthy; it only describes the checks that were available and run.

## Roadmap

- Windows integrity and servicing diagnostics
- Deeper SMART and filesystem checks
- Network adapter, DNS, route, and connectivity diagnostics
- WHEA and hardware sensor information
- Expanded driver inventory
- JSON and HTML report export

## Project

SystemWatch is an early preview and is under active development. Features are added incrementally and documented as they become available.

- **Language:** Rust 2021
- **Terminal UI:** Ratatui + Crossterm
- **License:** MIT

Bug reports, contributions, and feature suggestions are welcome.