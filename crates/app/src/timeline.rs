//! Clock text and selection fractions ↔ seconds.

use stemcraft_core::timerange::{parse_clock, TimeRange};

/// Shortest part of a song worth separating.
pub const MIN_SELECTION_SECS: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionError {
    BadFormat,
    TooShort,
}

/// `m:ss`, truncated to whole seconds.
pub fn format_clock(secs: f64) -> String {
    let total = secs.max(0.0).floor() as u64;
    format!("{}:{:02}", total / 60, total % 60)
}

pub fn frac_to_secs(frac: f32, duration: f64) -> f64 {
    frac.clamp(0.0, 1.0) as f64 * duration
}

pub fn secs_to_frac(secs: f64, duration: f64) -> f32 {
    if duration <= 0.0 {
        return 0.0;
    }
    (secs / duration).clamp(0.0, 1.0) as f32
}

/// Parse the start / end fields (`SS`, `M:SS` or `H:MM:SS`) into a selection.
pub fn parse_selection(start: &str, end: &str, duration: f64) -> Result<(f32, f32), SelectionError> {
    let parse = |text: &str| parse_clock(text.trim()).map_err(|_| SelectionError::BadFormat);
    let start = parse(start)?.min(duration);
    let end = parse(end)?.min(duration);
    if end - start < MIN_SELECTION_SECS {
        return Err(SelectionError::TooShort);
    }
    Ok((secs_to_frac(start, duration), secs_to_frac(end, duration)))
}

pub fn check_selection(selection: (f32, f32), duration: f64) -> Result<(), SelectionError> {
    if (selection.1 - selection.0) as f64 * duration < MIN_SELECTION_SECS {
        Err(SelectionError::TooShort)
    } else {
        Ok(())
    }
}

pub fn selection_to_range(selection: (f32, f32), duration: f64) -> TimeRange {
    TimeRange {
        start: frac_to_secs(selection.0, duration),
        end: Some(frac_to_secs(selection.1, duration)),
    }
}

/// Decide whether the trim page's time fields have actually been edited by
/// the user since the app last wrote into them, and if so, parse them.
///
/// `last_written` is the `(start, end)` text the app itself most recently
/// put into the inputs (e.g. after a waveform drag, or after a previous
/// successful parse). Returns `None` when both fields still match that text
/// (nothing to apply — the caller should leave the selection and any error
/// state untouched), or `Some(parse_selection(start, end, duration))` when
/// either field differs from what was last written.
pub fn edited_selection(
    start: &str,
    end: &str,
    last_written: (&str, &str),
    duration: f64,
) -> Option<Result<(f32, f32), SelectionError>> {
    if start == last_written.0 && end == last_written.1 {
        return None;
    }
    Some(parse_selection(start, end, duration))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_minutes_and_seconds() {
        assert_eq!(format_clock(0.0), "0:00");
        assert_eq!(format_clock(75.9), "1:15");
        assert_eq!(format_clock(3600.0), "60:00");
        assert_eq!(format_clock(-3.0), "0:00");
    }

    #[test]
    fn converts_between_fraction_and_seconds() {
        assert_eq!(frac_to_secs(0.5, 200.0), 100.0);
        assert_eq!(frac_to_secs(1.5, 200.0), 200.0);
        assert_eq!(secs_to_frac(50.0, 200.0), 0.25);
        assert_eq!(secs_to_frac(500.0, 200.0), 1.0);
        assert_eq!(secs_to_frac(5.0, 0.0), 0.0);
    }

    #[test]
    fn parses_a_selection() {
        assert_eq!(parse_selection("0:50", "1:40", 200.0), Ok((0.25, 0.5)));
        // End past the song clamps to the end.
        assert_eq!(parse_selection("0", "9:00", 200.0), Ok((0.0, 1.0)));
        assert_eq!(parse_selection("abc", "1:00", 200.0), Err(SelectionError::BadFormat));
        assert_eq!(parse_selection("1:00", "1:00.5", 200.0), Err(SelectionError::TooShort));
        assert_eq!(parse_selection("2:00", "1:00", 200.0), Err(SelectionError::TooShort));
    }

    #[test]
    fn checks_selection_length() {
        assert_eq!(check_selection((0.0, 0.004), 200.0), Err(SelectionError::TooShort));
        assert_eq!(check_selection((0.0, 0.01), 200.0), Ok(()));
    }

    #[test]
    fn selection_becomes_a_time_range() {
        let r = selection_to_range((0.25, 0.5), 200.0);
        assert_eq!(r.start, 50.0);
        assert_eq!(r.end, Some(100.0));
    }

    #[test]
    fn edited_selection_is_none_when_text_matches_last_written() {
        assert_eq!(edited_selection("0:00", "3:20", ("0:00", "3:20"), 200.7), None);
    }

    #[test]
    fn edited_selection_parses_when_text_changed() {
        assert_eq!(
            edited_selection("0:50", "1:40", ("0:00", "3:20"), 200.0),
            Some(Ok((0.25, 0.5)))
        );
    }

    #[test]
    fn edited_selection_reports_bad_format_when_changed_text_is_invalid() {
        assert_eq!(
            edited_selection("nope", "1:40", ("0:00", "3:20"), 200.0),
            Some(Err(SelectionError::BadFormat))
        );
    }

    #[test]
    fn edited_selection_triggers_on_either_field_alone() {
        // Only the start field changed.
        assert_eq!(
            edited_selection("0:10", "3:20", ("0:00", "3:20"), 200.0),
            Some(parse_selection("0:10", "3:20", 200.0))
        );
        // Only the end field changed.
        assert_eq!(
            edited_selection("0:00", "3:00", ("0:00", "3:20"), 200.0),
            Some(parse_selection("0:00", "3:00", 200.0))
        );
    }
}
