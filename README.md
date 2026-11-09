# SystemWatch

Portable Windows system diagnostics with an interactive terminal interface.

> Status: early development / project bootstrap

SystemWatch aims to inspect as much of a Windows computer as can be checked safely and reliably, explain detected issues, and produce useful diagnostic reports. It is designed to run from a folder or removable drive without installation.

## Goals
- Portable executable; no mandatory installer.
- Interactive terminal UI with keyboard navigation and clear progress.
- Broad diagnostics across Windows, hardware, storage, drivers, services, processes, networking, security configuration, and event logs.
- Explain findings in plain language, distinguish confirmed faults from warnings and unavailable checks.
- Export reports in JSON and HTML.
- Work offline for core checks.
- Support multiple Windows generations where technically feasible, with compatibility tested per release.
- Modular architecture so diagnostic checks can be added independently.

## Safety principles
- Read-only by default: scanning must not change system configuration.
- Never delete files, stop services, edit the registry, repair filesystems, or alter security settings automatically.
- Any future repair action must be explicit, separately described, and confirmed by the operator.
- Do not label a condition as a fault without evidence; show the source and limitations of each finding.
- Avoid collecting or exporting personal data that is not needed for diagnostics.

## Planned technology
- Rust
- Ratatui for the terminal interface
- Crossterm for terminal input and rendering
- sysinfo for system/process metrics
- Windows APIs and native command-line tools for Windows-specific diagnostics
- Serde for structured data and JSON reports
- HTML and JSON report exporters

Exact dependency versions and the minimum supported Windows version will be decided during implementation and compatibility testing.

## Diagnostic areas
| Area | Planned checks |
|---|---|
| System overview | Windows edition/build, uptime, CPU, memory, hardware inventory |
| Hardware | Device status, available temperature/health sensors, hardware error indicators |
| Storage | Disk inventory, free space, supported SMART/health data, filesystem status |
| Windows integrity | System component and protected-file checks, servicing state |
| Drivers & devices | Driver metadata, device-manager problem codes |
| Services | Service state, failed starts, relevant configuration |
| Processes | Resource usage, responsiveness indicators, unusual resource consumption |
| Network | Adapters, addressing, DNS, routes, connectivity and latency |
| Security | Defender and Firewall status, selected security configuration |
| Event logs | Relevant system/application warnings and errors |
| Performance | Resource pressure and evidence-backed bottleneck indicators |

Not every check is available on every Windows version or hardware configuration. Unsupported and permission-restricted checks must be reported as such, not as successful.

## Result severity
- **Critical** — strong evidence of an urgent fault or risk.
- **Warning** — a condition worth investigating; not necessarily a failure.
- **Info** — useful context or a recommendation.
- **Healthy** — the check completed and found no issue within its scope.
- **Unavailable** — unsupported, inaccessible, or not applicable.

## Development roadmap
1. Bootstrap the Rust project and define module boundaries.
2. Build the interactive terminal shell and navigation.
3. Implement system inventory and live resource overview.
4. Add read-only diagnostic modules incrementally.
5. Add event-log correlation and evidence-based explanations.
6. Add JSON and HTML report export.
7. Test on supported Windows versions and hardware configurations.
8. Package and verify portable releases.

## Development environment
Intended local working directory: C:\\projects\\SystemWatch

The repository is the source of truth for project documentation and code. Keep this README updated whenever architecture, compatibility, safety behavior, or roadmap status changes.

## Current status
The repository has been created and the initial project documentation is in place. Implementation has not yet been bootstrapped. The development computer must be reachable before local environment inspection, building, and runtime testing can be performed.
