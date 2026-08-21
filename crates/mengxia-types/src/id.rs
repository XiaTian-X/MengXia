use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use uuid::{Builder, Uuid, Variant, Version};

use crate::{IdGenerationError, ValueError};

const MAX_UUID_V7_MILLIS: u128 = (1_u128 << 48) - 1;

/// A type-safe, opaque, non-nil RFC UUIDv7 identity.
pub struct Id<T> {
    uuid: Uuid,
    marker: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    /// Generate an ID from the current OS clock and entropy source.
    pub fn try_new() -> Result<Self, IdGenerationError> {
        Self::try_new_with_sources(
            || {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| IdGenerationError::ClockBeforeUnixEpoch)
            },
            |random| getrandom::fill(random).map_err(|_| IdGenerationError::EntropyUnavailable),
        )
    }

    /// Parse the exact canonical database bytes and validate the UUID contract.
    pub fn from_bytes(bytes: [u8; 16]) -> Result<Self, ValueError> {
        let uuid = Uuid::from_bytes(bytes);
        if uuid.is_nil()
            || uuid.get_variant() != Variant::RFC4122
            || uuid.get_version() != Some(Version::SortRand)
        {
            return Err(ValueError::InvalidId);
        }
        Ok(Self {
            uuid,
            marker: PhantomData,
        })
    }

    /// Return the exact canonical database bytes.
    #[must_use]
    pub const fn to_bytes(self) -> [u8; 16] {
        self.uuid.into_bytes()
    }

    fn try_new_with_sources<Clock, Entropy>(
        clock: Clock,
        entropy: Entropy,
    ) -> Result<Self, IdGenerationError>
    where
        Clock: FnOnce() -> Result<Duration, IdGenerationError>,
        Entropy: FnOnce(&mut [u8; 10]) -> Result<(), IdGenerationError>,
    {
        let elapsed = clock()?;
        let millis = elapsed.as_millis();
        if millis > MAX_UUID_V7_MILLIS {
            return Err(IdGenerationError::TimestampOutOfRange);
        }

        let mut random = [0_u8; 10];
        entropy(&mut random)?;
        let uuid = Builder::from_unix_timestamp_millis(millis as u64, &random).into_uuid();
        Ok(Self {
            uuid,
            marker: PhantomData,
        })
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid
    }
}

impl<T> Eq for Id<T> {}

impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.uuid.cmp(&other.uuid)
    }
}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uuid.hash(state);
    }
}

impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Id({self})")
    }
}

impl<T> fmt::Display for Id<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.uuid.hyphenated(), formatter)
    }
}

impl<T> FromStr for Id<T> {
    type Err = ValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 36 || !value.is_ascii() {
            return Err(ValueError::InvalidId);
        }
        let uuid = Uuid::parse_str(value).map_err(|_| ValueError::InvalidId)?;
        if uuid.hyphenated().to_string() != value {
            return Err(ValueError::InvalidId);
        }
        Self::from_bytes(uuid.into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_generation_seams_map_each_failure() {
        let clock_error = Id::<()>::try_new_with_sources(
            || Err(IdGenerationError::ClockBeforeUnixEpoch),
            |_| Ok(()),
        );
        assert_eq!(clock_error, Err(IdGenerationError::ClockBeforeUnixEpoch));

        let range_error =
            Id::<()>::try_new_with_sources(|| Ok(Duration::from_millis(1_u64 << 48)), |_| Ok(()));
        assert_eq!(range_error, Err(IdGenerationError::TimestampOutOfRange));

        let entropy_error = Id::<()>::try_new_with_sources(
            || Ok(Duration::from_millis(1)),
            |_| Err(IdGenerationError::EntropyUnavailable),
        );
        assert_eq!(entropy_error, Err(IdGenerationError::EntropyUnavailable));
    }

    #[test]
    fn deterministic_generation_builds_an_rfc_uuid_v7() {
        let id = Id::<()>::try_new_with_sources(
            || Ok(Duration::from_millis(1_700_000_000_000)),
            |random| {
                *random = [0x5a; 10];
                Ok(())
            },
        )
        .expect("fixed valid sources produce an ID");

        let uuid = Uuid::from_bytes(id.to_bytes());
        assert_eq!(uuid.get_variant(), Variant::RFC4122);
        assert_eq!(uuid.get_version(), Some(Version::SortRand));
    }
}
