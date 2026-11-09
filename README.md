# SystemWatch

**Portable Windows diagnostics. Clear results. No automatic repairs.**

SystemWatch is an interactive terminal application for inspecting a Windows PC, understanding potential problems, and collecting useful diagnostic information — without silently changing system settings.

## What works today

The early preview (0.1.0) currently provides:

- **System overview:** Windows version, computer name, CPU load and physical memory usage.
- **Storage inventory:** mounted volumes, filesystem types, used and total capacity.
- **Device diagnostics:** checks Plug and Play devices for reported Device Manager problem codes.
- **Microsoft Defender:** reads the real-time protection status.
- **Windows Firewall:** checks whether the returned firewall profiles are enabled.
- **Interactive terminal UI:** four sections, live inventory refresh, and a manual diagnostics scan.

The diagnostics section reports each check as **HEALTHY**, **WARNING**, or **UNAVAILABLE**. A result that could not be read is never treated as a successful check.

## Preview

```text
┌──────────────────────────────────────────────────────────────────┐
│ SYSTEMWATCH   1: Overview   2: Diagnostics   3: Storage   4: About│
│                                                                  │
│ Read-only Windows checks                                        │
│                                                                  │
│ [HEALTHY] Devices & drivers                                      │
│   No device problem codes reported                               │
│                                                                  │
│ [HEALTHY] Microsoft Defender                                     │
│   Real-time protection is enabled                                │
│                                                                  │
│ [HEALTHY] Windows Firewall                                       │
│   All returned firewall profiles are enabled                     │
└──────────────────────────────────────────────────────────────────┘
```
<sub>Illustrative example only. Your computer may return different results or mark a check unavailable.</sub>

## Run

A packaged public release is not published yet. To build from source:

```powershell
git clone https://github.com/Caracoto4/SystemWatch.git
cd SystemWatch
cargo run
```

Requirements for building: Rust toolchain and Cargo. The exact supported Windows versions are still being tested.

## Keyboard controls

| Key | Action |
|---|---|
| `1`–`4` | Open a section |
| `←` / `→` or `Tab` | Switch sections |
| `R` | Refresh live inventory |
| `S` | Run diagnostics (Diagnostics section) |
| `Q` or `Esc` | Quit |

## Safety first

SystemWatch is designed to be **read-only by default**.

- It does not automatically repair Windows or modify system settings.
- It does not delete files, stop services, or change security configuration.
- It does not label a PC or process as malicious based on guesswork.
- Checks that cannot run are shown as unavailable, not healthy.

Some future diagnostics may require administrator permissions. The app will explain when a check is unavailable and why.

## Planned diagnostics

- Windows integrity and servicing state
- Disk health, SMART indicators and filesystem information
- Windows services and relevant event logs
- Network adapters, DNS, routes and connectivity
- Hardware sensor and WHEA indicators
- Detailed driver inventory
- JSON and HTML diagnostic reports

The exact Windows versions supported will be documented after compatibility testing. Older Windows versions are not claimed as supported yet.

## Project status

SystemWatch is under active development. This preview is an initial diagnostic toolkit, not a complete PC health scanner. Features are added incrementally and tested before being marked as available.

- **Language:** Rust
- **Interface:** Ratatui terminal UI
- **License:** MIT

Contributions, bug reports and feature suggestions are welcome.