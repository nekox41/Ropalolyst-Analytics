# Architecture

## Principles

1. The core engine is the source of truth. UI code never parses raw EE.log text.
2. Every mission run is an independent session. A later launch must never be merged into an earlier run.
3. Preserve raw log timestamps and source excerpts for traceability.
4. Event signatures have confidence levels: confirmed, strong candidate, and experimental.
5. Missing events are valid input. The state machine must tolerate partial logs and interrupted sessions.
6. Never infer a gameplay event solely from a UI animation unless that signal is explicitly marked as video-derived.

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

### ropalolyst-desktop

Presentation only:

- Live run view and timer.
- Encounter phase timeline.
- Session history and comparisons.
- EE.log source selection and diagnostics.
- Privacy controls and sanitized export.

## Initial event model

The following signatures are research findings, not yet a complete parser contract:

| Normalized event | Candidate source signal | Confidence |
|---|---|---|
| Mission started | GameRules state transition to `SS_STARTED` | Confirmed across supplied samples |
| Mission succeeded | `EndOfMatch.lua: Mission Succeeded` | Confirmed across supplied samples |
| Mission ending | `SS_ENDING` without a prior success/failure event | Candidate abort/failure boundary |
| Boss arena intro | `RopalolystGasArenaIntro01_cin.fbx` | Strong candidate |
| Crash animation start | `TowerCrashA_cin.fbx` | Strong candidate |
| Crash animation end | `TowerCrashEnd_cin.fbx` | Strong candidate |
| Boss defeated | `DRopaKillLaserNatah` / objective-complete transmission | Strong candidate; verify against more runs |

Do not treat transmission playback as proof of a precise health-zero frame until synchronized footage confirms the relationship.

## Timing model

- Mission elapsed time is measured from the first `SS_STARTED` event to the mission-success event.
- Loading requests such as `Host_StartMatch` are diagnostic milestones, not the official timer start.
- Store both raw log timestamps and derived durations.
- Video timestamps are a separate clock domain. Align them only with an explicit synchronization offset and retain the offset uncertainty.

## Video/log synchronization notes

The supplied 2m28 recording is approximately 190.88 seconds long and includes lobby/loading, gameplay, and the results screen. The in-game results panel displays 2m28. This allows a coarse visual alignment, but exact event matching requires frame-level inspection and the matching raw log.

## Privacy

Raw logs and recordings must not be committed. Tests should use sanitized fixtures. The sanitizer should replace account IDs, IP addresses, machine identifiers, and paths while preserving event ordering and timing.
