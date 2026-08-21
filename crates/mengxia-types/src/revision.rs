use std::fmt;
use std::str::FromStr;

use crate::{RevisionOverflow, ValueError};

/// An optimistic-concurrency revision counter.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RevisionNo(u64);

impl RevisionNo {
    pub const INITIAL: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn checked_next(self) -> Result<Self, RevisionOverflow> {
        match self.0.checked_add(1) {
            Some(next) => Ok(Self(next)),
            None => Err(RevisionOverflow),
        }
    }
}

impl fmt::Display for RevisionNo {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for RevisionNo {
    type Err = ValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty()
            || value.len() > 20
            || !value.is_ascii()
            || !value.bytes().all(|byte| byte.is_ascii_digit())
            || (value.len() > 1 && value.starts_with('0'))
        {
            return Err(ValueError::InvalidRevision);
        }
        value
            .parse::<u64>()
            .map(Self)
            .map_err(|_| ValueError::InvalidRevision)
    }
}
