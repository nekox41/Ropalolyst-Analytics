# Ropalolyst Analytics

A lightweight Rust console application for analyzing Warframe Ropalolyst runs.

## First-release scope

The first release is a **console application only**. It uses a polished terminal dashboard (TUI), not a desktop GUI.

- Incrementally read the Warframe EE.log file.
- Identify individual mission sessions, including repeated entries and aborted runs.
- Reconstruct Ropalolyst encounter milestones from verified log events.
- Display a live timer, current phase, event timeline, and parser status.
- Save completed runs and calculate best, average, and split statistics.
- Ship as a standalone Windows x64 executable.

## Technology

- **Rust** for the core engine and executable.
- **Ratatui + Crossterm** for the terminal dashboard.
- **SQLite** for local run history (planned).
- **Serde** for configuration and data serialization.
- **GitHub Actions** for Windows builds and release artifacts.

No Python, Node.js, browser engine, or GUI runtime is required.

## Workspace

- `crates/ropalolyst-core`: log ingestion, parsing, normalized events, mission segmentation, encounter state machine, timing, and persistence.
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

EE.log files can contain account identifiers, IP addresses, machine details, and local paths. Do not commit raw player logs or recordings. Use sanitized fixtures with identifiers replaced while preserving event order and timing.
