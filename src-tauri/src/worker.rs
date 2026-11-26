
use crate::{lock, overlay, Shared, Status};
use chrono::Utc;
use ropalolyst_core::{
    parse_clock_anchor, parse_log_line, Analyzer, LogClockAnchor, LogTailer, MissionStatus,
    RunStore, phase_splits,
};
use std::{io, path::PathBuf, sync::Arc, thread, time::Duration};
use tauri::AppHandle;

fn store_completed(
    store: &RunStore,
    session: &ropalolyst_core::MissionSession,
    generation: &str,
    anchor: Option<LogClockAnchor>,
) -> Result<(), String> {
    let started = anchor
        .and_then(|clock| clock.to_utc(session.started_at))
        .map(|utc| utc.to_rfc3339());
    store
        .save_with_utc(session, generation, started.as_deref())
        .map_err(|error| error.to_string())
}

/// Replay all existing logs for historical results; only show the overlay for
/// sessions whose final log event is recent in UTC. No memory injection is used.
pub fn run(shared: Arc<Shared>, app: AppHandle) {
    let mut current_path = PathBuf::new();
    let mut tailer: Option<LogTailer> = None;
    let mut analyzer = Analyzer::default();
    let mut anchor: Option<LogClockAnchor> = None;
    let mut generation = String::new();
    let mut first_scan = true;
    let mut revision = 0_u64;
    let mut previous_offset = 0_u64;

    loop {
        let path = lock(&shared.path).clone();
        if current_path != path || tailer.is_none() {
            current_path = path.clone();
            tailer = Some(LogTailer::new(path.clone()));
            analyzer = Analyzer::default();
            anchor = None;
            generation.clear();
            first_scan = true;
            previous_offset = 0;
            revision += 1;
            overlay::hide(&app, &shared);
        }

        let mut status = Status {
            mode: "waiting".into(),
            path: path.to_string_lossy().into_owned(),
            revision,
            ..Status::default()
        };

        let tail = tailer.as_mut().expect("tailer initialized");
        match tail.read_new_lines() {
            Ok(lines) => {
                let next_generation = tail.generation_id();
                let replaced = previous_offset > 0
                    && (tail.offset() < previous_offset || next_generation != generation);
                if replaced {
                    // A previous game process has disappeared. Do not infer a
                    // successful run, and never continue its overlay timer.
                    if let Some(mut session) = analyzer.active.take() {
                        session.status = MissionStatus::Aborted;
                        session.ended_at = analyzer.last_event.as_ref().map(|event| event.timestamp);
                        if let Err(error) = store_completed(
                            &lock(&shared.store), &session, &generation, anchor,
                        ) {
                            status.error = error;
                        } else {
                            revision += 1;
                        }
                    }
                    analyzer = Analyzer::default();
                    anchor = None;
                    first_scan = true;
                    overlay::hide(&app, &shared);
                }
                generation = next_generation;
                previous_offset = tail.offset();
                let was_replay = first_scan;
                let mut finished: Option<(MissionStatus, f64)> = None;

                for line in lines {
                    if let Some(clock) = parse_clock_anchor(&line) {
                        anchor = Some(clock);
                    }
                    if let Some(event) = parse_log_line(&line) {
                        let before = analyzer.completed.len();
                        analyzer.ingest(event);
                        for session in analyzer.completed.iter().skip(before) {
                            match store_completed(&lock(&shared.store), session, &generation, anchor) {
                                Ok(()) => revision += 1,
                                Err(error) => status.error = error,
                            }
                            if let Some(seconds) = session.duration_seconds() {
                                finished = Some((session.status, seconds));
                            }
                        }
                    }
                }
                first_scan = false;
                status.mode = "live".into();
                status.offset = tail.offset();

                if let (Some(session), Some(clock), Some(last)) = (
                    analyzer.active.as_ref(), anchor, analyzer.last_event.as_ref(),
                ) {
                    let event_time = clock.to_utc(last.timestamp);
                    let start_time = clock.to_utc(session.started_at);
                    if let (Some(event_time), Some(start_time)) = (event_time, start_time) {
                        let event_age_ms = Utc::now()
                            .signed_duration_since(event_time)
                            .num_milliseconds();
                        let elapsed_ms = Utc::now()
                            .signed_duration_since(start_time)
                            .num_milliseconds();
                        // Avoid reviving stale replay sessions, and cap clock
                        // extrapolation if a game log stops emitting events.
                        if (0..=45_000).contains(&event_age_ms) && elapsed_ms >= 0 {
                            let seconds = elapsed_ms as f64 / 1000.0;
                            status.active = true;
                            status.elapsed_seconds = Some(seconds);
                            status.splits = phase_splits(session);
                            let key = format!("{}:{:.3}", generation, session.started_at.0);
                            lock(&shared.overlay).live(key, seconds);
                        } else {
                            overlay::hide(&app, &shared);
                        }
                    }
                }

                if let Some((outcome, seconds)) = finished.filter(|_| !was_replay) {
                    if outcome == MissionStatus::Succeeded {
                        lock(&shared.overlay).finished(seconds);
                    } else {
                        overlay::hide(&app, &shared);
                    }
                }
                if analyzer.active.is_none() && finished.is_none() {
                    lock(&shared.overlay).tick();
                }
            }
            Err(error) => {
                status.mode = if error.kind() == io::ErrorKind::NotFound {
                    "waiting".into()
                } else {
                    "retry".into()
                };
                status.error = error.to_string();
                overlay::hide(&app, &shared);
            }
        }

        status.revision = revision;
        *lock(&shared.status) = status;
        {
            let mut runtime = lock(&shared.overlay);
            if let Err(error) = overlay::sync(&app, &mut runtime) {
                eprintln!("Overlay: {error}");
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
}
