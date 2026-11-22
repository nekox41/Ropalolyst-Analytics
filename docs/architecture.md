# Architecture

## Product scope

Version 0.1 is a **console-only application**. It must not introduce a desktop GUI, WebView, browser runtime, or frontend build chain.

Use Ratatui and Crossterm to provide a polished terminal dashboard with fixed regions, clear color semantics, and low-overhead redraws.

## Principles

1. The core engine is the source of truth. TUI code never parses raw EE.log text.
2. Every mission run is an independent session. A later launch must never be merged into an earlier run.
3. Preserve raw log timestamps and source excerpts for traceability.
4. Event signatures have confidence levels: confirmed, strong candidate, and experimental.
5. Missing events are valid input. The state machine must tolerate partial logs and interrupted sessions.
6. The display must distinguish confirmed events from provisional detections.
7. Keep terminal rendering separate from the parser and encounter state machine.

## Workspace

### ropalolyst-core

Planned modules:

- `ingest`: tail/read EE.log safely, handle truncation and rotation.
- `parser`: identify source log records and extract timestamps.
- `signatures`: versioned rules for known Warframe log signatures.
- `event`: normalized typed events with source provenance and confidence.
- `session`: split repeated mission launches into independent sessions.
- `encounter`: Ropalolyst phase state machine.
- `timing`: elapsed time and phase splits based on source log timestamps.
- `storage`: local history and schema migrations.

### ropalolyst-cli

Presentation and application orchestration only:

- Live timer and mission status.
- Encounter milestone timeline.
- Log source and watcher status.
- Recent runs, best time, average, and split comparison.
- Settings and diagnostics.

## Console UX requirements

- Use alternate-screen mode and restore the terminal on normal exit.
- Refresh the dashboard in place; do not flood the terminal with one line per event.
- Use colors consistently: green for success/connected, yellow for waiting or uncertain, red for failure, cyan for active timing, gray for inactive/unverified.
- Show milliseconds for splits and elapsed time.
- Keep a compact event timeline with timestamps and clear confidence indicators.
- Support a useful minimum terminal size and show a resize hint when the terminal is too small.
- Keep diagnostic logs on stderr or in a separate file, never mixed into the TUI frame.
- Provide a plain-text fallback mode later if terminal capabilities are unavailable.

## Initial event model

These signatures are research findings, not a complete parser contract:

| Normalized event | Candidate source signal | Confidence |
|---|---|---|
| Mission started | GameRules transition to `SS_STARTED` | Confirmed across supplied samples |
| Mission succeeded | `EndOfMatch.lua: Mission Succeeded` | Confirmed across supplied samples |
| Mission ending | `SS_ENDING` without prior success/failure | Candidate abort/failure boundary |
| Boss arena intro | `RopalolystGasArenaIntro01_cin.fbx` | Strong candidate |
| Crash animation start | `TowerCrashA_cin.fbx` | Strong candidate |
| Crash animation end | `TowerCrashEnd_cin.fbx` | Strong candidate |
| Boss defeated | `DRopaKillLaserNatah` / objective-complete transmission | Strong candidate; verify against more runs |

Do not treat transmission playback as proof of the precise health-zero frame until synchronized footage confirms the relationship.

## Timing model

- Mission elapsed time is measured from the first `SS_STARTED` event to the mission-success event.
- Loading requests such as `Host_StartMatch` are diagnostic milestones, not the official timer start.
- Store raw log timestamps and derived durations.
- Video timestamps are a separate clock domain. Align them only with an explicit synchronization offset and retain the offset uncertainty.

## Video/log synchronization: first 2m28 run

The supplied MP4 is 1920x1080 at 60 fps and approximately 190.88 seconds long. It includes pre-mission/lobby footage, loading, the complete encounter, and post-mission results.

The results overlay is visible at approximately video time 163.5s and displays an in-game mission duration of 2m28s. The matching log session has `SS_STARTED` at 42.312s and `Mission Succeeded` at 191.218s, a log-clock duration of 148.906s.

This gives a provisional mapping of approximately `log_timestamp ≈ video_timestamp + 27.7s`, with roughly ±0.5s uncertainty. It is not millisecond-accurate synchronization.

## Privacy

Raw logs and recordings must not be committed. Tests should use sanitized fixtures. The sanitizer should replace account IDs, IP addresses, machine identifiers, and paths while preserving event ordering and timing.
