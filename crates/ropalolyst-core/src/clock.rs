use chrono::{DateTime, NaiveDateTime, Utc};

use crate::LogTimestamp;

/// Maps Warframe's process-relative log clock to UTC using the startup diagnostic line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogClockAnchor {
    pub log_timestamp: LogTimestamp,
    pub utc_time: DateTime<Utc>,
}

impl LogClockAnchor {
    pub fn to_utc(&self, timestamp: LogTimestamp) -> Option<DateTime<Utc>> {
        let delta_seconds = timestamp.0 - self.log_timestamp.0;
        if !delta_seconds.is_finite() {
            return None;
        }
        let delta_millis = (delta_seconds * 1_000.0).round();
        if delta_millis < i64::MIN as f64 || delta_millis > i64::MAX as f64 {
            return None;
        }
        Some(self.utc_time + chrono::Duration::milliseconds(delta_millis as i64))
    }
}

/// Parses lines such as:
/// `0.376 Sys [Diag]: Current time: Sun Nov 22 16:00:48 2026 [UTC: Sun Nov 22 08:00:48 2026]`
pub fn parse_clock_anchor(line: &str) -> Option<LogClockAnchor> {
    let log_timestamp = line
        .split_whitespace()
        .next()?
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .map(LogTimestamp)?;

    let utc_value = line.split_once("[UTC:")?.1.split_once(']')?.0.trim();
    let naive = NaiveDateTime::parse_from_str(utc_value, "%a %b %d %H:%M:%S %Y").ok()?;
    Some(LogClockAnchor {
        log_timestamp,
        utc_time: naive.and_utc(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_utc_anchor_and_maps_log_time() {
        let line = "0.376 Sys [Diag]: Current time: Sun Nov 22 16:00:48 2026 [UTC: Sun Nov 22 08:00:48 2026]";
        let anchor = parse_clock_anchor(line).unwrap();
        assert_eq!(anchor.log_timestamp, LogTimestamp(0.376));
        assert_eq!(anchor.utc_time.to_rfc3339(), "2026-11-22T08:00:48+00:00");
        assert_eq!(
            anchor.to_utc(LogTimestamp(10.376)).unwrap().to_rfc3339(),
            "2026-11-22T08:00:58+00:00"
        );
    }

    #[test]
    fn rejects_malformed_or_non_finite_anchor() {
        assert!(parse_clock_anchor("0.2 Sys [Diag]: Current time: not a date [UTC: nope]").is_none());
        assert!(parse_clock_anchor("NaN Sys [Diag]: Current time: Sun Nov 22 16:00:48 2026 [UTC: Sun Nov 22 08:00:48 2026]").is_none());
    }
}
