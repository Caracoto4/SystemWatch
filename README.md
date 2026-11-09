# SystemWatch

**Portable Windows diagnostics. Clear results. No automatic repairs.**

SystemWatch is an interactive terminal application for inspecting a Windows PC, understanding potential problems, and collecting useful diagnostic information — without silently changing system settings.

<pre>
┌──────────────────────────────────────────────────────────────┐
│  SYSTEMWATCH     1: Overview   2: Diagnostics   3: Storage   │
│                                                              │
│  System                                                       │
│  Host: YOUR-PC                                                │
│  OS: Windows 11                                               │
│                                                              │
│  CPU average   ███████░░░░░░░░░░░░  34%                       │
│  Memory        ███████████░░░░░░░  56%                        │
│                                                              │
│  Notes                                                        │
│  Live inventory is informational — not a full health verdict.│
└──────────────────────────────────────────────────────────────┘
</pre>
<sub>Illustrative layout preview; actual values come from the computer being scanned.</sub>

## What works today

This is an early preview (0.1.0). The current build can:

- Show Windows name, version, kernel version, and computer name.
- Display live average CPU utilization and physical memory usage.
- List mounted volumes, filesystem types, and volume capacities.
- Refresh system inventory automatically and on demand.
- Navigate a compact terminal interface using the keyboard.

More diagnostic modules are planned. The interface clearly marks checks that are not implemented yet.

## Run

1. Download the latest release executable when a release is published.
2. Open Windows Terminal or PowerShell.
3. Run `systemwatch.exe`.

For now, developers can build it from source:

```powershell
git clone https://github.com/Caracoto4/SystemWatch.git
cd SystemWatch
cargo run
```

## Keyboard controls

| Key | Action |
|---|---|
| `1`–`4` | Open a section |
| `←` / `→` or `Tab` | Switch sections |
| `R` | Refresh inventory |
| `Q` or `Esc` | Quit |

## Safety first

SystemWatch is designed to be **read-only by default**.

- It does not automatically repair Windows or modify system settings.
- It does not delete files, stop services, or change security configuration.
- It distinguishes confirmed findings from warnings and unavailable checks.
- A check that could not run is never presented as a successful check.

Some future diagnostics may require administrator permissions. The app will explain when a check is unavailable and why.

## Planned diagnostics

- Windows integrity and servicing state
- Hardware and device health indicators
- Driver and Device Manager problem codes
- Disk health and filesystem information
- Windows services and relevant event logs
- Network adapters, DNS, routes, and connectivity
- Microsoft Defender and Windows Firewall status
- JSON and HTML diagnostic reports

The exact Windows versions supported will be documented after compatibility testing. Older Windows versions are not claimed as supported yet.

## Project status

SystemWatch is under active development. The current preview is an inventory dashboard, not a complete PC health scanner. Features will be added incrementally and tested before being marked as available.

- **Language:** Rust
- **Interface:** Ratatui terminal UI
- **License:** MIT

Contributions, bug reports, and feature suggestions are welcome.