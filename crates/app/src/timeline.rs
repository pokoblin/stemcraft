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

/// Apply the trim page's time fields to a selection, parsing only the
/// field(s) the user actually edited since the app last wrote into the
/// inputs.
///
/// `last_written` is the `(start, end)` text the app itself most recently
/// put into the inputs (e.g. after a waveform drag, or after a previous
/// successful apply). `current` is the selection as it stands right now
/// (exact fractions, not rounded to whole seconds like the display text).
///
/// A field whose text still equals its `last_written` text is untouched, so
/// its side of `current` is kept exactly rather than re-parsed from the
/// (whole-second) display text — parsing it would needlessly truncate a
/// fractional selection. Only a field whose text differs is parsed; a bad
/// format there is reported as `BadFormat` even if the other field is fine.
/// When both fields are untouched (including a field that was edited and
/// then reverted back to the remembered text), this returns `Ok(current)`
/// unchanged — callers should treat that as clearing any stale error, since
/// the current selection is by definition valid already.
///
/// The combined result is still checked against the minimum selection
/// length, so an edit that makes the selection too short reports
/// `TooShort` same as `parse_selection`.
pub fn edited_selection(
    start: &str,
    end: &str,
    last_written: (&str, &str),
    current: (f32, f32),
    duration: f64,
) -> Result<(f32, f32), SelectionError> {
    let start_changed = start != last_written.0;
    let end_changed = end != last_written.1;
    if !start_changed && !end_changed {
        return Ok(current);
    }
    let parse = |text: &str| parse_clock(text.trim()).map_err(|_| SelectionError::BadFormat);
    let start_secs = if start_changed { parse(start)? } else { frac_to_secs(current.0, duration) }.min(duration);
    let end_secs = if end_changed { parse(end)? } else { frac_to_secs(current.1, duration) }.min(duration);
    if end_secs - start_secs < MIN_SELECTION_SECS {
        return Err(SelectionError::TooShort);
    }
    Ok((secs_to_frac(start_secs, duration), secs_to_frac(end_secs, duration)))
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
    fn edited_selection_keeps_current_exactly_when_both_fields_unchanged() {
        // 200.7s song: the current selection holds an exact fraction that
        // whole-second display text ("0:00" / "3:20") cannot represent.
        assert_eq!(
            edited_selection("0:00", "3:20", ("0:00", "3:20"), (0.3, 1.0), 200.7),
            Ok((0.3, 1.0))
        );
    }

    #[test]
    fn edited_selection_clears_to_current_when_a_bad_edit_is_reverted() {
        // The user typed something invalid earlier (which set input_error),
        // then retyped the exact text the app last wrote. From this
        // function's point of view that's indistinguishable from "never
        // touched it" — Ok(current) tells the caller to clear the error.
        assert_eq!(
            edited_selection("0:00", "3:20", ("0:00", "3:20"), (0.0, 1.0), 200.7),
            Ok((0.0, 1.0))
        );
    }

    #[test]
    fn edited_selection_parses_only_the_changed_start_field() {
        let duration = 200.7;
        let current = (0.3, 1.0);
        // End text ("3:20") matches last_written, so its exact current
        // fraction is kept rather than reparsed from truncated display text.
        let expected_end = current.1;
        let expected_start = secs_to_frac(10.0, duration);
        assert_eq!(
            edited_selection("0:10", "3:20", ("0:00", "3:20"), current, duration),
            Ok((expected_start, expected_end))
        );
    }

    #[test]
    fn edited_selection_parses_only_the_changed_end_field() {
        let duration = 200.7;
        let current = (0.3, 1.0);
        let expected_start = current.0;
        let expected_end = secs_to_frac(100.0, duration);
        assert_eq!(
            edited_selection("0:00", "1:40", ("0:00", "3:20"), current, duration),
            Ok((expected_start, expected_end))
        );
    }

    #[test]
    fn edited_selection_reports_bad_format_for_changed_invalid_text() {
        assert_eq!(
            edited_selection("nope", "3:20", ("0:00", "3:20"), (0.0, 1.0), 200.0),
            Err(SelectionError::BadFormat)
        );
    }

    #[test]
    fn edited_selection_reports_too_short_when_an_edit_shrinks_it() {
        // Start is untouched at 0s; the end is edited down to 0.5s.
        assert_eq!(
            edited_selection("0:00", "0:00.5", ("0:00", "3:20"), (0.0, 1.0), 200.0),
            Err(SelectionError::TooShort)
        );
    }
}
