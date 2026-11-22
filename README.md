# Ropalolyst Analytics

A lightweight native Windows desktop application for analyzing Warframe Ropalolyst runs.

## Project goals

- Parse Warframe EE logs incrementally and identify individual mission sessions.
- Reconstruct Ropalolyst encounter phases from verified log events.
- Provide live timers, phase splits, run history, and post-run analysis.
- Ship as a standalone Windows executable without requiring Python or Node.js.
- Keep the analysis engine independent from the desktop UI.

## Technology direction

- **Rust** for the parsing, event model, state machine, timing, and persistence layers.
- **egui/eframe** for a native immediate-mode desktop UI. This avoids embedding a browser runtime and keeps distribution simpler than a web-based desktop shell.
- **SQLite** for local run history, introduced when persistence is implemented.
- **GitHub Actions** for Windows builds and release artifacts.

The first target is Windows x64. The core library should remain platform-independent where practical.

## Architecture

The repository is a Cargo workspace:

- `crates/ropalolyst-core`: log ingestion, parsing, event normalization, mission segmentation, encounter state machine, timing, and analysis.
- `crates/ropalolyst-desktop`: native desktop application and presentation layer.

The UI must not contain parsing rules or infer game state directly. It consumes typed events and snapshots from the core library.

## Development status

The repository is being initialized. Event signatures are provisional until verified against multiple sanitized EE.log samples and synchronized gameplay footage.

## Build

Once the initial workspace is in place:

```powershell
cargo build --release
```

The Windows executable will be produced under:

```
target/release/ropalolyst-analytics.exe
```

## Data and privacy

EE.log files can contain account identifiers, IP addresses, machine details, and other sensitive values. Do not commit raw player logs or recordings. Use sanitized fixtures with identifiers replaced while preserving event order and timing.
