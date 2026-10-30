//! Wall-clock timestamps on the wire.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// A point in time as Unix epoch milliseconds (UTC). Serialized as an integer,
/// which stays exact in JavaScript for the next ~285,000 years.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Timestamp(u64);

impl Timestamp {
    /// Creates a timestamp from Unix epoch milliseconds.
    #[must_use]
    pub const fn from_unix_millis(millis: u64) -> Self {
        Self(millis)
    }

    /// Unix epoch milliseconds.
    #[must_use]
    pub const fn unix_millis(self) -> u64 {
        self.0
    }

    /// The current system time.
    #[must_use]
    pub fn now() -> Self {
        SystemTime::now().into()
    }

    /// Converts to a [`SystemTime`].
    #[must_use]
    pub fn to_system_time(self) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(self.0)
    }
}

impl From<SystemTime> for Timestamp {
    /// Times before 1970 saturate to zero.
    fn from(time: SystemTime) -> Self {
        let millis = time.duration_since(UNIX_EPOCH).map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        });
        Self(millis)
    }
}

impl From<Timestamp> for SystemTime {
    fn from(timestamp: Timestamp) -> Self {
        timestamp.to_system_time()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_as_integer_millis() {
        let ts = Timestamp::from_unix_millis(1_767_225_600_123);
        assert_eq!(serde_json::to_string(&ts).unwrap(), "1767225600123");
        assert_eq!(
            serde_json::from_str::<Timestamp>("1767225600123").unwrap(),
            ts
        );
    }

    #[test]
    fn system_time_round_trip() {
        let ts = Timestamp::from_unix_millis(1_800_000_000_000);
        assert_eq!(Timestamp::from(ts.to_system_time()), ts);
        assert_eq!(
            Timestamp::from(UNIX_EPOCH - Duration::from_secs(5)),
            Timestamp(0)
        );
        assert!(Timestamp::now() > Timestamp::from_unix_millis(1_767_225_600_000));
    }
}
