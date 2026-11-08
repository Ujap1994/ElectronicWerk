//! Calendar dates without external dependencies (UTC).

use std::time::{SystemTime, UNIX_EPOCH};

/// A UTC date and time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTime {
    /// Year.
    pub year: i64,
    /// Month `1..=12`.
    pub month: u8,
    /// Day `1..=31`.
    pub day: u8,
    /// Hour `0..=23`.
    pub hour: u8,
    /// Minute `0..=59`.
    pub minute: u8,
    /// Second `0..=59`.
    pub second: u8,
}

impl DateTime {
    /// Converts seconds since the Unix epoch.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "every value is reduced to its calendar range first"
    )]
    pub const fn from_unix(seconds: i64) -> Self {
        let days = seconds.div_euclid(86_400);
        let secs = seconds.rem_euclid(86_400);
        // Howard Hinnant's civil_from_days.
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
        Self {
            year,
            month: month as u8,
            day: day as u8,
            hour: (secs / 3_600) as u8,
            minute: (secs % 3_600 / 60) as u8,
            second: (secs % 60) as u8,
        }
    }

    /// The current UTC time.
    #[must_use]
    pub fn now() -> Self {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(0));
        Self::from_unix(seconds)
    }

    /// `YYYY-MM-DD`.
    #[must_use]
    pub fn date_string(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// `YYYY-MM-DDTHH:MM:SSZ`.
    #[must_use]
    pub fn iso_string(&self) -> String {
        format!(
            "{}T{:02}:{:02}:{:02}Z",
            self.date_string(),
            self.hour,
            self.minute,
            self.second
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dates() {
        assert_eq!(DateTime::from_unix(0).iso_string(), "1970-01-01T00:00:00Z");
        assert_eq!(
            DateTime::from_unix(1_709_210_096).iso_string(),
            "2024-02-29T12:34:56Z"
        );
        assert_eq!(DateTime::from_unix(-1).iso_string(), "1969-12-31T23:59:59Z");
        assert_eq!(
            DateTime::from_unix(4_102_444_800).date_string(),
            "2100-01-01"
        );
        assert!(DateTime::now().year >= 2026);
    }
}
