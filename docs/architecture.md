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

## Video/log synchronization: first 2m28 run

The supplied MP4 is 1920x1080 at 60 fps and approximately 190.88 seconds long. It includes pre-mission/lobby footage, loading, the complete encounter, and post-mission results.

The results overlay is first visible at approximately video time 163.5s and displays an in-game mission duration of 2m28s. The matching log session has `SS_STARTED` at 42.312s and `Mission Succeeded` at 191.218s, a log-clock duration of 148.906s.

This gives an initial clock mapping of approximately:

`log_timestamp ≈ video_timestamp + 27.7s`

Treat this as a provisional alignment with roughly ±0.5s uncertainty, not a millisecond-accurate synchronization. The in-game result timer is displayed only to whole seconds and the first visible results frame is not necessarily the exact success-event frame.

Approximate visual cross-checks using that mapping:

| Log event time | Estimated video time | Visual observation |
|---:|---:|---|
| 69.831s | 42.1s | Arena intro sequence begins |
| 87.399s | 59.7s | Boss is in the arena and the fight is underway |
| 98.866s | 71.2s | First crash/down sequence |
| 129.554s | 101.9s | Second crash/down sequence |
| 159.200s | 131.5s | Third crash/down sequence |
| 172.014s | 144.3s | Large impact/explosion and apparent finishing sequence |
| 191.218s | 163.5s | Mission results overlay is visible |

The footage visually supports the three repeated crash/down cycles and places the objective-complete candidate near the finishing explosion. It does not, by itself, prove the exact frame at which Boss health reaches zero. Keep that event as a strong candidate until further recordings/logs confirm it.

## Privacy

Raw logs and recordings must not be committed. Tests should use sanitized fixtures. The sanitizer should replace account IDs, IP addresses, machine identifiers, and paths while preserving event ordering and timing.
