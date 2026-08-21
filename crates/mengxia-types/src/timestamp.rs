use std::fmt;
use std::str::FromStr;

use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

use crate::ValueError;

const NANOS_PER_SECOND: i128 = 1_000_000_000;

/// A canonical UTC instant with nanosecond precision.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Timestamp(OffsetDateTime);

impl Timestamp {
    pub fn from_unix_seconds_nanos(seconds: i64, nanos: u32) -> Result<Self, ValueError> {
        if nanos > 999_999_999 {
            return Err(ValueError::InvalidTimestamp);
        }
        let total_nanos = i128::from(seconds) * NANOS_PER_SECOND + i128::from(nanos);
        let timestamp = OffsetDateTime::from_unix_timestamp_nanos(total_nanos)
            .map_err(|_| ValueError::InvalidTimestamp)?;
        Self::from_validated_utc(timestamp)
    }

    #[must_use]
    pub const fn unix_seconds(self) -> i64 {
        self.0.unix_timestamp()
    }

    #[must_use]
    pub const fn subsec_nanoseconds(self) -> u32 {
        self.0.nanosecond()
    }

    fn from_validated_utc(timestamp: OffsetDateTime) -> Result<Self, ValueError> {
        if timestamp.offset() != UtcOffset::UTC || !(1..=9999).contains(&timestamp.year()) {
            return Err(ValueError::InvalidTimestamp);
        }
        Ok(Self(timestamp))
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Timestamp({self})")
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let canonical = self.0.format(&Rfc3339).map_err(|_| fmt::Error)?;
        formatter.write_str(&canonical)
    }
}

impl FromStr for Timestamp {
    type Err = ValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if !(20..=30).contains(&value.len()) || !value.is_ascii() {
            return Err(ValueError::InvalidTimestamp);
        }
        let timestamp =
            OffsetDateTime::parse(value, &Rfc3339).map_err(|_| ValueError::InvalidTimestamp)?;
        let timestamp = Self::from_validated_utc(timestamp)?;
        if timestamp.to_string() != value {
            return Err(ValueError::InvalidTimestamp);
        }
        Ok(timestamp)
    }
}
