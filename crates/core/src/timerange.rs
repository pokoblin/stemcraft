//! Time ranges such as `1:30-3:00`, `90-180`, `1:30-` or `-2:00`.

use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeRange {
    pub start: f64,
    /// `None` means "until the end of the file".
    pub end: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseRangeError(String);

impl fmt::Display for ParseRangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseRangeError {}

/// `SS`, `M:SS` or `H:MM:SS` (seconds may be fractional) → seconds.
pub fn parse_clock(text: &str) -> Result<f64, ParseRangeError> {
    let invalid = || ParseRangeError(format!("invalid time: {text:?}"));
    let parts: Vec<&str> = text.trim().split(':').collect();
    if parts.len() > 3 {
        return Err(invalid());
    }
    parts.iter().try_fold(0.0, |acc, part| {
        let part = part.trim();
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return Err(invalid());
        }
        let value: f64 = part.parse().map_err(|_| invalid())?;
        Ok(acc * 60.0 + value)
    })
}

impl FromStr for TimeRange {
    type Err = ParseRangeError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (left, right) = text.split_once('-').ok_or_else(|| {
            ParseRangeError(format!("invalid range {text:?}, expected e.g. 1:30-3:00"))
        })?;
        let (left, right) = (left.trim(), right.trim());
        let start = if left.is_empty() {
            0.0
        } else {
            parse_clock(left)?
        };
        let end = if right.is_empty() {
            None
        } else {
            Some(parse_clock(right)?)
        };
        if end.is_some_and(|end| end <= start) {
            return Err(ParseRangeError("range end must be after its start".into()));
        }
        Ok(TimeRange { start, end })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clock_formats() {
        assert_eq!(parse_clock("45").unwrap(), 45.0);
        assert_eq!(parse_clock("1:30").unwrap(), 90.0);
        assert_eq!(parse_clock("1:02:03").unwrap(), 3723.0);
        assert_eq!(parse_clock("2:05.5").unwrap(), 125.5);
    }

    #[test]
    fn parses_ranges() {
        let r = |s: &str| s.parse::<TimeRange>().unwrap();
        assert_eq!(
            r("1:30-3:00"),
            TimeRange {
                start: 90.0,
                end: Some(180.0)
            }
        );
        assert_eq!(
            r("90 - 180"),
            TimeRange {
                start: 90.0,
                end: Some(180.0)
            }
        );
        assert_eq!(
            r("1:30-"),
            TimeRange {
                start: 90.0,
                end: None
            }
        );
        assert_eq!(
            r("-2:00"),
            TimeRange {
                start: 0.0,
                end: Some(120.0)
            }
        );
    }

    #[test]
    fn rejects_bad_ranges() {
        for bad in [
            "3:00-1:30",
            "abc",
            "1:30",
            "1::2-3",
            "",
            "1:2:3:4-5",
            "-1-2",
        ] {
            assert!(
                bad.parse::<TimeRange>().is_err(),
                "{bad:?} should be rejected"
            );
        }
    }
}
