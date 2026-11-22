# Ropalolyst Analytics

A lightweight Rust console application for analyzing Warframe Ropalolyst runs.

## First-release scope

The first release is a **console application only**. It uses a polished terminal dashboard (TUI), not a desktop GUI.

- Auto-discover `%LOCALAPPDATA%\\Warframe\\EE.log` for the current Windows user.
- Incrementally read complete log lines and recover from truncation.
- Identify mission starts, success/failure boundaries, and split repeated sessions.
- Display a live timer, recognized event timeline, log watcher status, and confidence labels.
- Persist completed mission sessions in a local SQLite database and calculate best/average times.
- Ship as a standalone Windows x64 executable.

Ropalolyst cinematic/defeat signatures are currently surfaced as candidates, not authoritative phase transitions. Their exact semantics require validation against sanitized logs from multiple real runs before phase timing is considered release-grade.

## Technology

- **Rust** for the core engine and executable.
- **Ratatui + Crossterm** for the terminal dashboard.
- **SQLite** for local run history (planned).
- **Serde** for configuration and data serialization.
- **GitHub Actions** for Windows builds and release artifacts.

No Python, Node.js, browser engine, or GUI runtime is required.

## Workspace

- `crates/ropalolyst-core`: log ingestion, parsing, normalized events, mission segmentation, event aggregation, timing, and SQLite persistence.
- `crates/ropalolyst-cli`: terminal UI and application entry point.

The terminal UI must not parse raw EE.log text or infer game state. It consumes typed events and snapshots from the core library.

## Build

Install the Rust stable toolchain, then run:

```powershell
cargo build --release -p ropalolyst-cli
```

Executable:

```
target/release/ropalolyst-analytics.exe
```

## Controls

- `Q` or `Esc`: quit.

## Data and privacy

EE.log files can contain account identifiers, IP addresses, machine details, and local paths. The application stores normalized mission events and timings only; it does not copy raw log lines into SQLite. Do not commit raw player logs or recordings. Use sanitized fixtures with identifiers replaced while preserving event order and timing.
