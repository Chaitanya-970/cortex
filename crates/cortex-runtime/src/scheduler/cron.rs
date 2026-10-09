//! Standard 5-field cron expression parsing and schedule calculations.
//!
//! Provides deterministic next-execution time calculations for 5-field cron schedules
//! and one-shot timers without busy polling loops.

use chrono::{DateTime, Datelike, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

/// Error returned when a cron expression or schedule fails to parse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CronParseError {
    /// Expression does not contain the expected number of fields.
    #[error("invalid field count in cron expression '{0}': expected 5 fields")]
    InvalidFieldCount(String),

    /// A field contains an invalid value or syntax.
    #[error("invalid value '{value}' in field '{field}': {reason}")]
    InvalidField {
        /// Name of the cron field.
        field: &'static str,
        /// Field string value.
        value: String,
        /// Explanation of failure.
        reason: String,
    },

    /// Value is out of the acceptable range for the field.
    #[error("value {val} out of range for {field} ({min}..={max})")]
    ValueOutOfRange {
        /// Name of the cron field.
        field: &'static str,
        /// Invalid value.
        val: u32,
        /// Minimum permitted value.
        min: u32,
        /// Maximum permitted value.
        max: u32,
    },

    /// An unrecognized cron macro was encountered.
    #[error("unrecognized cron macro '{0}'")]
    UnknownMacro(String),

    /// Timestamp parsing failed for one-shot timer.
    #[error("invalid one-shot timestamp '{0}': {1}")]
    InvalidTimestamp(String, String),
}

/// Represents a parsed single field in a 5-field cron expression.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CronField {
    /// The set of matching numeric values for this field.
    pub values: BTreeSet<u32>,
    /// Whether the field was specified as a wildcard (`*`).
    pub is_wildcard: bool,
}

impl CronField {
    /// Check if a value matches this field.
    pub fn matches(&self, val: u32) -> bool {
        self.values.contains(&val)
    }

    /// Check if this field is a wildcard.
    pub fn is_wildcard(&self) -> bool {
        self.is_wildcard
    }
}

/// Standard 5-field cron expression representation.
///
/// Fields: `minute` (0-59), `hour` (0-23), `day_of_month` (1-31), `month` (1-12), `day_of_week` (0-7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CronExpression {
    /// Raw expression string.
    raw: String,
    /// Minute field (0-59).
    pub minute: CronField,
    /// Hour field (0-23).
    pub hour: CronField,
    /// Day of month field (1-31).
    pub day_of_month: CronField,
    /// Month field (1-12).
    pub month: CronField,
    /// Day of week field (0-7, where both 0 and 7 are Sunday).
    pub day_of_week: CronField,
}

impl CronExpression {
    /// Parse a 5-field cron expression or macro into a [`CronExpression`].
    pub fn parse(expr: &str) -> Result<Self, CronParseError> {
        let trimmed = expr.trim();

        // Support standard cron macros
        let normalized = match trimmed.to_ascii_lowercase().as_str() {
            "@yearly" | "@annually" => "0 0 1 1 *",
            "@monthly" => "0 0 1 * *",
            "@weekly" => "0 0 * * 0",
            "@daily" | "@midnight" => "0 0 * * *",
            "@hourly" => "0 * * * *",
            macro_str if macro_str.starts_with('@') => {
                return Err(CronParseError::UnknownMacro(macro_str.to_string()));
            }
            _ => trimmed,
        };

        let parts: Vec<&str> = normalized.split_whitespace().collect();
        if parts.len() != 5 {
            return Err(CronParseError::InvalidFieldCount(expr.to_string()));
        }

        let minute = parse_field(parts[0], "minute", 0, 59, parse_minute_value)?;
        let hour = parse_field(parts[1], "hour", 0, 23, parse_hour_value)?;
        let day_of_month = parse_field(parts[2], "day of month", 1, 31, parse_day_of_month_value)?;
        let month = parse_field(parts[3], "month", 1, 12, parse_month_value)?;
        let mut day_of_week = parse_field(parts[4], "day of week", 0, 7, parse_day_of_week_value)?;

        // Standardize day of week: both 0 and 7 mean Sunday
        if day_of_week.values.contains(&7) {
            day_of_week.values.insert(0);
        }
        if day_of_week.values.contains(&0) {
            day_of_week.values.insert(7);
        }

        Ok(Self {
            raw: trimmed.to_string(),
            minute,
            hour,
            day_of_month,
            month,
            day_of_week,
        })
    }

    /// Access the raw expression string.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Check if a specific day of month and day of week match according to POSIX cron rules.
    pub fn matches_day(&self, dom: u32, dow: u32) -> bool {
        let dom_matches = self.day_of_month.matches(dom);
        let dow_matches = self.day_of_week.matches(dow);

        if self.day_of_month.is_wildcard() && self.day_of_week.is_wildcard() {
            true
        } else if self.day_of_month.is_wildcard() {
            dow_matches
        } else if self.day_of_week.is_wildcard() {
            dom_matches
        } else {
            // Both specified: POSIX specifies union (OR)
            dom_matches || dow_matches
        }
    }

    /// Check if a given [`DateTime<Utc>`] matches this cron expression.
    pub fn is_match(&self, dt: &DateTime<Utc>) -> bool {
        self.minute.matches(dt.minute())
            && self.hour.matches(dt.hour())
            && self.month.matches(dt.month())
            && self.matches_day(dt.day(), dt.weekday().num_days_from_sunday())
    }

    /// Calculate the next scheduled execution time strictly after `from`.
    ///
    /// Jumps forward hierarchically across months, days, hours, and minutes
    /// to avoid busy polling loops.
    pub fn next_run_after(&self, from: &DateTime<Utc>) -> Option<DateTime<Utc>> {
        // Start candidate at the beginning of the next minute
        let mut candidate = from
            .with_second(0)?
            .with_nanosecond(0)?
            .checked_add_signed(chrono::Duration::minutes(1))?;

        let max_year = from.year() + 5;

        while candidate.year() <= max_year {
            // 1. Check month
            if !self.month.matches(candidate.month()) {
                if let Some(&next_m) = self.month.values.range(candidate.month() + 1..).next() {
                    candidate = set_date_time(candidate.year(), next_m, 1, 0, 0)?;
                } else {
                    let next_year = candidate.year() + 1;
                    let first_m = *self.month.values.iter().next()?;
                    candidate = set_date_time(next_year, first_m, 1, 0, 0)?;
                }
                continue;
            }

            // 2. Check day of month & day of week
            let dow = candidate.weekday().num_days_from_sunday();
            if !self.matches_day(candidate.day(), dow) {
                // Advance to beginning of next day
                candidate = candidate
                    .with_hour(0)?
                    .with_minute(0)?
                    .checked_add_signed(chrono::Duration::days(1))?;
                continue;
            }

            // 3. Check hour
            if !self.hour.matches(candidate.hour()) {
                if let Some(&next_h) = self.hour.values.range(candidate.hour() + 1..).next() {
                    candidate = candidate.with_hour(next_h)?.with_minute(0)?;
                } else {
                    candidate = candidate
                        .with_hour(0)?
                        .with_minute(0)?
                        .checked_add_signed(chrono::Duration::days(1))?;
                }
                continue;
            }

            // 4. Check minute
            if !self.minute.matches(candidate.minute()) {
                if let Some(&next_min) = self.minute.values.range(candidate.minute() + 1..).next() {
                    candidate = candidate.with_minute(next_min)?;
                } else {
                    candidate = candidate
                        .with_minute(0)?
                        .checked_add_signed(chrono::Duration::hours(1))?;
                }
                continue;
            }

            // All constraints satisfied
            return Some(candidate);
        }

        None
    }
}

impl fmt::Display for CronExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl FromStr for CronExpression {
    type Err = CronParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// Helper to safely create a [`DateTime<Utc>`] with given components.
fn set_date_time(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> Option<DateTime<Utc>> {
    use chrono::{NaiveDate, TimeZone};
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    let time = date.and_hms_opt(hour, minute, 0)?;
    Some(Utc.from_utc_datetime(&time))
}

/// Schedule definition supporting recurring cron expressions or one-shot timers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Schedule {
    /// Recurring schedule defined by a 5-field cron expression.
    Cron(CronExpression),
    /// One-shot trigger executed once at a specific UTC timestamp.
    OneShot(DateTime<Utc>),
}

impl Schedule {
    /// Parse a schedule string into a [`Schedule`].
    ///
    /// Accepts 5-field cron expressions, cron macros, or ISO 8601 timestamps
    /// (with optional `@once ` or `@at ` prefixes).
    pub fn parse(s: &str) -> Result<Self, CronParseError> {
        let trimmed = s.trim();

        // Check for one-shot prefix
        let ts_str = if let Some(rest) = trimmed.strip_prefix("@once ") {
            Some(rest.trim())
        } else if let Some(rest) = trimmed.strip_prefix("@at ") {
            Some(rest.trim())
        } else if trimmed.starts_with('@') {
            None
        } else {
            // Check if entire string parses as RFC 3339 datetime
            if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
                return Ok(Self::OneShot(dt.with_timezone(&Utc)));
            }
            None
        };

        if let Some(ts) = ts_str {
            let dt = DateTime::parse_from_rfc3339(ts)
                .map_err(|e| CronParseError::InvalidTimestamp(ts.to_string(), e.to_string()))?;
            return Ok(Self::OneShot(dt.with_timezone(&Utc)));
        }

        // Parse as cron expression
        let cron = CronExpression::parse(trimmed)?;
        Ok(Self::Cron(cron))
    }

    /// Calculate the next run time strictly after `from`.
    pub fn next_run_after(&self, from: &DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self {
            Self::Cron(cron) => cron.next_run_after(from),
            Self::OneShot(target) => {
                if target > from {
                    Some(*target)
                } else {
                    None
                }
            }
        }
    }

    /// Check if this schedule is a one-shot timer.
    pub fn is_one_shot(&self) -> bool {
        matches!(self, Self::OneShot(_))
    }
}

impl fmt::Display for Schedule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cron(cron) => write!(f, "{}", cron),
            Self::OneShot(dt) => write!(f, "{}", dt.to_rfc3339()),
        }
    }
}

impl FromStr for Schedule {
    type Err = CronParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

// ---------------------------------------------------------------------------
// Internal Cron Field Parsing Helpers
// ---------------------------------------------------------------------------

fn parse_field<F>(
    raw: &str,
    field_name: &'static str,
    min: u32,
    max: u32,
    val_parser: F,
) -> Result<CronField, CronParseError>
where
    F: Fn(&str) -> Result<u32, CronParseError>,
{
    let is_wildcard = raw == "*";
    let mut values = BTreeSet::new();

    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(CronParseError::InvalidField {
                field: field_name,
                value: raw.to_string(),
                reason: "empty item in list".to_string(),
            });
        }

        if part == "*" {
            for v in min..=max {
                values.insert(v);
            }
        } else if let Some(step_str) = part.strip_prefix("*/") {
            let step: u32 = step_str.parse().map_err(|_| CronParseError::InvalidField {
                field: field_name,
                value: part.to_string(),
                reason: format!("invalid step value '{}'", step_str),
            })?;
            if step == 0 {
                return Err(CronParseError::InvalidField {
                    field: field_name,
                    value: part.to_string(),
                    reason: "step cannot be zero".to_string(),
                });
            }
            let mut current = min;
            while current <= max {
                values.insert(current);
                current += step;
            }
        } else if part.contains('/') {
            let slash_parts: Vec<&str> = part.split('/').collect();
            if slash_parts.len() != 2 {
                return Err(CronParseError::InvalidField {
                    field: field_name,
                    value: part.to_string(),
                    reason: "invalid step syntax".to_string(),
                });
            }
            let step: u32 = slash_parts[1]
                .parse()
                .map_err(|_| CronParseError::InvalidField {
                    field: field_name,
                    value: part.to_string(),
                    reason: format!("invalid step value '{}'", slash_parts[1]),
                })?;
            if step == 0 {
                return Err(CronParseError::InvalidField {
                    field: field_name,
                    value: part.to_string(),
                    reason: "step cannot be zero".to_string(),
                });
            }

            let (range_start, range_end) =
                parse_range(slash_parts[0], field_name, min, max, &val_parser)?;
            let mut current = range_start;
            while current <= range_end {
                values.insert(current);
                current += step;
            }
        } else if part.contains('-') {
            let (range_start, range_end) = parse_range(part, field_name, min, max, &val_parser)?;
            for v in range_start..=range_end {
                values.insert(v);
            }
        } else {
            let val = val_parser(part)?;
            if val < min || val > max {
                return Err(CronParseError::ValueOutOfRange {
                    field: field_name,
                    val,
                    min,
                    max,
                });
            }
            values.insert(val);
        }
    }

    Ok(CronField {
        values,
        is_wildcard,
    })
}

fn parse_range<F>(
    raw: &str,
    field_name: &'static str,
    min: u32,
    max: u32,
    val_parser: &F,
) -> Result<(u32, u32), CronParseError>
where
    F: Fn(&str) -> Result<u32, CronParseError>,
{
    let parts: Vec<&str> = raw.split('-').collect();
    if parts.len() != 2 {
        return Err(CronParseError::InvalidField {
            field: field_name,
            value: raw.to_string(),
            reason: "invalid range syntax".to_string(),
        });
    }

    let start = val_parser(parts[0])?;
    let end = val_parser(parts[1])?;

    if start < min || start > max {
        return Err(CronParseError::ValueOutOfRange {
            field: field_name,
            val: start,
            min,
            max,
        });
    }
    if end < min || end > max {
        return Err(CronParseError::ValueOutOfRange {
            field: field_name,
            val: end,
            min,
            max,
        });
    }
    if start > end {
        return Err(CronParseError::InvalidField {
            field: field_name,
            value: raw.to_string(),
            reason: format!("range start {} is greater than end {}", start, end),
        });
    }

    Ok((start, end))
}

fn parse_minute_value(s: &str) -> Result<u32, CronParseError> {
    s.parse().map_err(|_| CronParseError::InvalidField {
        field: "minute",
        value: s.to_string(),
        reason: "must be an integer 0..59".to_string(),
    })
}

fn parse_hour_value(s: &str) -> Result<u32, CronParseError> {
    s.parse().map_err(|_| CronParseError::InvalidField {
        field: "hour",
        value: s.to_string(),
        reason: "must be an integer 0..23".to_string(),
    })
}

fn parse_day_of_month_value(s: &str) -> Result<u32, CronParseError> {
    s.parse().map_err(|_| CronParseError::InvalidField {
        field: "day of month",
        value: s.to_string(),
        reason: "must be an integer 1..31".to_string(),
    })
}

fn parse_month_value(s: &str) -> Result<u32, CronParseError> {
    if let Ok(v) = s.parse::<u32>() {
        return Ok(v);
    }
    match s.to_ascii_lowercase().as_str() {
        "jan" => Ok(1),
        "feb" => Ok(2),
        "mar" => Ok(3),
        "apr" => Ok(4),
        "may" => Ok(5),
        "jun" => Ok(6),
        "jul" => Ok(7),
        "aug" => Ok(8),
        "sep" => Ok(9),
        "oct" => Ok(10),
        "nov" => Ok(11),
        "dec" => Ok(12),
        _ => Err(CronParseError::InvalidField {
            field: "month",
            value: s.to_string(),
            reason: "unknown month name or number".to_string(),
        }),
    }
}

fn parse_day_of_week_value(s: &str) -> Result<u32, CronParseError> {
    if let Ok(v) = s.parse::<u32>() {
        return Ok(v);
    }
    match s.to_ascii_lowercase().as_str() {
        "sun" => Ok(0),
        "mon" => Ok(1),
        "tue" => Ok(2),
        "wed" => Ok(3),
        "thu" => Ok(4),
        "fri" => Ok(5),
        "sat" => Ok(6),
        _ => Err(CronParseError::InvalidField {
            field: "day of week",
            value: s.to_string(),
            reason: "unknown weekday name or number".to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_parse_valid_cron_expressions() {
        let every_minute = CronExpression::parse("* * * * *").unwrap();
        assert!(every_minute.minute.is_wildcard);

        let step_minutes = CronExpression::parse("*/15 * * * *").unwrap();
        assert_eq!(
            step_minutes
                .minute
                .values
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![0, 15, 30, 45]
        );

        let named_fields = CronExpression::parse("0 9 * JAN-MAR MON-FRI").unwrap();
        assert_eq!(
            named_fields.hour.values.iter().cloned().collect::<Vec<_>>(),
            vec![9]
        );
        assert_eq!(
            named_fields
                .month
                .values
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(
            named_fields
                .day_of_week
                .values
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn test_parse_cron_macros() {
        let daily = CronExpression::parse("@daily").unwrap();
        assert_eq!(
            daily.minute.values.iter().cloned().collect::<Vec<_>>(),
            vec![0]
        );
        assert_eq!(
            daily.hour.values.iter().cloned().collect::<Vec<_>>(),
            vec![0]
        );

        let hourly = CronExpression::parse("@hourly").unwrap();
        assert_eq!(
            hourly.minute.values.iter().cloned().collect::<Vec<_>>(),
            vec![0]
        );
        assert!(hourly.hour.is_wildcard);
    }

    #[test]
    fn test_next_run_calculation() {
        let expr = CronExpression::parse("*/15 10 * * *").unwrap();
        let from = Utc.with_ymd_and_hms(2026, 10, 9, 10, 2, 0).unwrap();
        let next = expr.next_run_after(&from).unwrap();

        assert_eq!(next, Utc.with_ymd_and_hms(2026, 10, 9, 10, 15, 0).unwrap());

        let next2 = expr.next_run_after(&next).unwrap();
        assert_eq!(next2, Utc.with_ymd_and_hms(2026, 10, 9, 10, 30, 0).unwrap());
    }

    #[test]
    fn test_one_shot_schedule() {
        let target = Utc.with_ymd_and_hms(2026, 12, 31, 23, 59, 0).unwrap();
        let sched = Schedule::parse("@once 2026-12-31T23:59:00Z").unwrap();
        assert!(sched.is_one_shot());

        let before = Utc.with_ymd_and_hms(2026, 10, 1, 0, 0, 0).unwrap();
        assert_eq!(sched.next_run_after(&before), Some(target));

        let after = Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap();
        assert_eq!(sched.next_run_after(&after), None);
    }

    #[test]
    fn test_invalid_cron_syntax() {
        assert!(CronExpression::parse("invalid").is_err());
        assert!(CronExpression::parse("60 * * * *").is_err());
        assert!(CronExpression::parse("* 25 * * *").is_err());
        assert!(CronExpression::parse("* * 32 * *").is_err());
        assert!(CronExpression::parse("* * * 13 *").is_err());
    }
}
