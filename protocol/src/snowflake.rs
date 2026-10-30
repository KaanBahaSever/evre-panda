//! 64-bit Snowflake IDs and their generator.
//!
//! Layout, most significant bit first:
//!
//! ```text
//! | 42 bits: ms since ERA_EPOCH_MS | 10 bits: worker | 12 bits: sequence |
//! ```
//!
//! IDs sort by creation time, so they double as a creation timestamp and as a
//! pagination cursor (`?after={id}`). 42 bits of milliseconds last until about
//! 2165. Stored in Postgres `BIGINT` (signed), values become negative after
//! 2^41 ms (about 2095); compare them as unsigned there.

use std::fmt;
use std::num::ParseIntError;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::serde_util::u64_string;
use crate::timestamp::Timestamp;

/// Era's custom epoch, 2026-01-01T00:00:00Z, in Unix milliseconds.
pub const ERA_EPOCH_MS: u64 = 1_767_225_600_000;

/// Bits used for the millisecond timestamp.
pub const TIMESTAMP_BITS: u32 = 42;
/// Bits used for the worker ID.
pub const WORKER_BITS: u32 = 10;
/// Bits used for the per-millisecond sequence.
pub const SEQUENCE_BITS: u32 = 12;

/// Highest valid worker ID (1023).
pub const MAX_WORKER_ID: u16 = (1 << WORKER_BITS) - 1;
/// Highest sequence number within one millisecond (4095).
pub const MAX_SEQUENCE: u16 = (1 << SEQUENCE_BITS) - 1;

const MAX_ELAPSED_MS: u64 = (1 << TIMESTAMP_BITS) - 1;
const WORKER_SHIFT: u32 = SEQUENCE_BITS;
const TIMESTAMP_SHIFT: u32 = SEQUENCE_BITS + WORKER_BITS;

/// A unique, time-ordered 64-bit ID.
///
/// On the wire it is a decimal string (`"1234567890123456789"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Snowflake(u64);

impl Snowflake {
    /// Wraps a raw ID.
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// The raw 64-bit value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Builds an ID from its parts. Each part is truncated to its bit width.
    #[must_use]
    pub const fn from_parts(elapsed_ms: u64, worker_id: u16, sequence: u16) -> Self {
        Self(
            ((elapsed_ms & MAX_ELAPSED_MS) << TIMESTAMP_SHIFT)
                | (((worker_id & MAX_WORKER_ID) as u64) << WORKER_SHIFT)
                | (sequence & MAX_SEQUENCE) as u64,
        )
    }

    /// The smallest ID that can be created at `timestamp`. Useful as a
    /// pagination bound ("messages after this time"). Times before the Era
    /// epoch map to zero.
    #[must_use]
    pub const fn first_at(timestamp: Timestamp) -> Self {
        Self::from_parts(timestamp.unix_millis().saturating_sub(ERA_EPOCH_MS), 0, 0)
    }

    /// Milliseconds since [`ERA_EPOCH_MS`].
    #[must_use]
    pub const fn elapsed_ms(self) -> u64 {
        self.0 >> TIMESTAMP_SHIFT
    }

    /// The worker that generated the ID.
    #[must_use]
    pub const fn worker_id(self) -> u16 {
        ((self.0 >> WORKER_SHIFT) as u16) & MAX_WORKER_ID
    }

    /// The sequence number within its millisecond.
    #[must_use]
    pub const fn sequence(self) -> u16 {
        (self.0 as u16) & MAX_SEQUENCE
    }

    /// When the ID was created.
    #[must_use]
    pub const fn timestamp(self) -> Timestamp {
        Timestamp::from_unix_millis(ERA_EPOCH_MS + self.elapsed_ms())
    }
}

impl From<u64> for Snowflake {
    fn from(raw: u64) -> Self {
        Self(raw)
    }
}

impl From<Snowflake> for u64 {
    fn from(id: Snowflake) -> Self {
        id.0
    }
}

impl fmt::Display for Snowflake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// Returned when a string is not a valid [`Snowflake`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid snowflake: {0}")]
pub struct ParseSnowflakeError(#[from] ParseIntError);

impl FromStr for Snowflake {
    type Err = ParseSnowflakeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}

impl Serialize for Snowflake {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        u64_string::serialize(self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for Snowflake {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        u64_string::deserialize(deserializer).map(Self)
    }
}

/// A source of wall-clock time for [`SnowflakeGenerator`].
pub trait Clock: Send + Sync {
    /// Current time in Unix epoch milliseconds.
    fn now_unix_millis(&self) -> u64;
}

/// The system clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix_millis(&self) -> u64 {
        Timestamp::from(SystemTime::now()).unix_millis()
    }
}

/// Returned when a worker ID does not fit in [`WORKER_BITS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("worker id {0} exceeds the maximum of {MAX_WORKER_ID}")]
pub struct InvalidWorkerId(pub u16);

/// Thread-safe, lock-free Snowflake generator for one worker.
///
/// IDs from one generator are strictly increasing. When 4096 IDs have been
/// issued within one millisecond, [`generate`](Self::generate) waits for the
/// next millisecond. If the clock steps backwards, the generator keeps using
/// the last timestamp (IDs never go backwards) and, once that millisecond's
/// sequence is used up, waits until the clock catches up.
///
/// Each running process needs its own worker ID; two generators sharing one
/// can produce duplicates.
#[derive(Debug)]
pub struct SnowflakeGenerator<C = SystemClock> {
    worker_id: u16,
    /// Last issued `(elapsed_ms << SEQUENCE_BITS) | sequence`.
    last: AtomicU64,
    clock: C,
}

impl SnowflakeGenerator<SystemClock> {
    /// A generator for `worker_id` using the system clock.
    pub fn new(worker_id: u16) -> Result<Self, InvalidWorkerId> {
        Self::with_clock(worker_id, SystemClock)
    }
}

impl<C: Clock> SnowflakeGenerator<C> {
    /// A generator for `worker_id` using a custom clock.
    pub fn with_clock(worker_id: u16, clock: C) -> Result<Self, InvalidWorkerId> {
        if worker_id > MAX_WORKER_ID {
            return Err(InvalidWorkerId(worker_id));
        }
        Ok(Self {
            worker_id,
            last: AtomicU64::new(0),
            clock,
        })
    }

    /// The worker ID embedded in every generated ID.
    #[must_use]
    pub fn worker_id(&self) -> u16 {
        self.worker_id
    }

    /// Generates the next ID.
    pub fn generate(&self) -> Snowflake {
        let max_sequence = u64::from(MAX_SEQUENCE);
        loop {
            let now = self.elapsed_now();
            let last = self.last.load(Ordering::Acquire);
            let last_ms = last >> SEQUENCE_BITS;

            let next = if now > last_ms {
                now << SEQUENCE_BITS
            } else if last & max_sequence < max_sequence {
                last + 1
            } else {
                wait_past(last_ms, now);
                continue;
            };

            if self
                .last
                .compare_exchange_weak(last, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Snowflake(
                    ((next >> SEQUENCE_BITS) << TIMESTAMP_SHIFT)
                        | (u64::from(self.worker_id) << WORKER_SHIFT)
                        | (next & max_sequence),
                );
            }
        }
    }

    fn elapsed_now(&self) -> u64 {
        self.clock
            .now_unix_millis()
            .saturating_sub(ERA_EPOCH_MS)
            .min(MAX_ELAPSED_MS)
    }
}

/// Blocks until the clock has (probably) moved past `last_ms`.
fn wait_past(last_ms: u64, now: u64) {
    let behind = last_ms.saturating_sub(now);
    if behind == 0 {
        // Same millisecond: the next one is at most 1 ms away.
        thread::yield_now();
    } else {
        // The clock stepped backwards; sleep until it catches up.
        thread::sleep(Duration::from_millis(behind));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;

    use super::*;

    /// A clock the test controls.
    #[derive(Clone)]
    struct ManualClock(Arc<AtomicU64>);

    impl ManualClock {
        fn at(unix_ms: u64) -> Self {
            Self(Arc::new(AtomicU64::new(unix_ms)))
        }

        fn set(&self, unix_ms: u64) {
            self.0.store(unix_ms, Ordering::SeqCst);
        }
    }

    impl Clock for ManualClock {
        fn now_unix_millis(&self) -> u64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    /// Returns `base` for the first `calls_before_step` calls, then `base + 1`.
    struct SteppingClock {
        calls: AtomicU64,
        calls_before_step: u64,
        base: u64,
    }

    impl Clock for SteppingClock {
        fn now_unix_millis(&self) -> u64 {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if call < self.calls_before_step {
                self.base
            } else {
                self.base + 1
            }
        }
    }

    #[test]
    fn epoch_is_2026_01_01_utc() {
        // 56 years since 1970, 14 of them leap years.
        let days = 56 * 365 + 14;
        assert_eq!(ERA_EPOCH_MS, days * 86_400_000);
    }

    #[test]
    fn layout_round_trips_parts() {
        let id = Snowflake::from_parts(123_456_789, 513, 4_000);
        assert_eq!(id.elapsed_ms(), 123_456_789);
        assert_eq!(id.worker_id(), 513);
        assert_eq!(id.sequence(), 4_000);
        assert_eq!(
            id.timestamp(),
            Timestamp::from_unix_millis(ERA_EPOCH_MS + 123_456_789)
        );
    }

    #[test]
    fn layout_bit_positions() {
        assert_eq!(Snowflake::from_parts(0, 0, 1).get(), 1);
        assert_eq!(Snowflake::from_parts(0, 1, 0).get(), 1 << 12);
        assert_eq!(Snowflake::from_parts(1, 0, 0).get(), 1 << 22);
        let max = Snowflake::from_parts(MAX_ELAPSED_MS, MAX_WORKER_ID, MAX_SEQUENCE);
        assert_eq!(max.get(), u64::MAX);
        // Out-of-range parts are truncated, never bleed into other fields.
        assert_eq!(
            Snowflake::from_parts(0, u16::MAX, 0).worker_id(),
            MAX_WORKER_ID
        );
        assert_eq!(Snowflake::from_parts(0, 0, u16::MAX).worker_id(), 0);
    }

    #[test]
    fn ids_order_by_time_first() {
        let earlier = Snowflake::from_parts(1_000, MAX_WORKER_ID, MAX_SEQUENCE);
        let later = Snowflake::from_parts(1_001, 0, 0);
        assert!(earlier < later);
        assert!(Snowflake::from_parts(5, 0, 1) < Snowflake::from_parts(5, 0, 2));
    }

    #[test]
    fn first_at_bounds_ids_of_that_millisecond() {
        let ts = Timestamp::from_unix_millis(ERA_EPOCH_MS + 42);
        let bound = Snowflake::first_at(ts);
        assert_eq!(bound.timestamp(), ts);
        assert!(bound <= Snowflake::from_parts(42, 0, 0));
        assert!(bound > Snowflake::from_parts(41, MAX_WORKER_ID, MAX_SEQUENCE));
        assert_eq!(Snowflake::first_at(Timestamp::from_unix_millis(0)).get(), 0);
    }

    #[test]
    fn display_and_from_str() {
        let id = Snowflake::new(1_234_567_890_123_456_789);
        assert_eq!(id.to_string(), "1234567890123456789");
        assert_eq!("1234567890123456789".parse::<Snowflake>().unwrap(), id);
        assert!("".parse::<Snowflake>().is_err());
        assert!("abc".parse::<Snowflake>().is_err());
        assert!("-1".parse::<Snowflake>().is_err());
        assert!("18446744073709551616".parse::<Snowflake>().is_err());
        assert_eq!(u64::from(id), id.get());
        assert_eq!(Snowflake::from(7_u64), Snowflake::new(7));
    }

    #[test]
    fn serde_uses_decimal_strings() {
        let id = Snowflake::new(u64::MAX);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"18446744073709551615\"");
        assert_eq!(serde_json::from_str::<Snowflake>(&json).unwrap(), id);
    }

    #[test]
    fn serde_rejects_numbers_and_garbage() {
        assert!(serde_json::from_str::<Snowflake>("123").is_err());
        assert!(serde_json::from_str::<Snowflake>("\"12a\"").is_err());
        assert!(serde_json::from_str::<Snowflake>("\"\"").is_err());
        assert!(serde_json::from_str::<Snowflake>("null").is_err());
    }

    #[test]
    fn rejects_invalid_worker_id() {
        assert!(SnowflakeGenerator::new(MAX_WORKER_ID).is_ok());
        assert_eq!(
            SnowflakeGenerator::new(MAX_WORKER_ID + 1).unwrap_err(),
            InvalidWorkerId(1024)
        );
    }

    #[test]
    fn generated_ids_carry_worker_and_current_time() {
        let clock = ManualClock::at(ERA_EPOCH_MS + 5_000);
        let generator = SnowflakeGenerator::with_clock(77, clock).unwrap();
        let id = generator.generate();
        assert_eq!(id.worker_id(), 77);
        assert_eq!(id.elapsed_ms(), 5_000);
        assert_eq!(id.sequence(), 0);
        assert_eq!(generator.generate().sequence(), 1);
        assert_eq!(generator.worker_id(), 77);
    }

    #[test]
    fn system_clock_ids_are_recent() {
        let id = SnowflakeGenerator::new(1).unwrap().generate();
        let age = Timestamp::now().unix_millis() - id.timestamp().unix_millis();
        assert!(age < 60_000, "id is {age} ms old");
    }

    #[test]
    fn sequence_resets_on_new_millisecond() {
        let clock = ManualClock::at(ERA_EPOCH_MS + 10);
        let generator = SnowflakeGenerator::with_clock(1, clock.clone()).unwrap();
        generator.generate();
        generator.generate();
        clock.set(ERA_EPOCH_MS + 11);
        let id = generator.generate();
        assert_eq!((id.elapsed_ms(), id.sequence()), (11, 0));
    }

    #[test]
    fn sequence_overflow_waits_for_next_millisecond() {
        let per_ms = u64::from(MAX_SEQUENCE) + 1;
        let clock = SteppingClock {
            calls: AtomicU64::new(0),
            // One clock read per ID, plus one that finds the sequence exhausted.
            calls_before_step: per_ms + 1,
            base: ERA_EPOCH_MS + 1_000,
        };
        let generator = SnowflakeGenerator::with_clock(3, clock).unwrap();
        let ids: Vec<_> = (0..=per_ms).map(|_| generator.generate()).collect();

        for (seq, id) in ids[..ids.len() - 1].iter().enumerate() {
            assert_eq!(id.elapsed_ms(), 1_000);
            assert_eq!(u64::from(id.sequence()), seq as u64);
        }
        let overflow = ids[ids.len() - 1];
        assert_eq!((overflow.elapsed_ms(), overflow.sequence()), (1_001, 0));
        assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn clock_going_backwards_never_reorders_ids() {
        let clock = ManualClock::at(ERA_EPOCH_MS + 1_000);
        let generator = SnowflakeGenerator::with_clock(1, clock.clone()).unwrap();
        let before = generator.generate();
        clock.set(ERA_EPOCH_MS + 900);
        let after = generator.generate();
        assert!(after > before);
        assert_eq!(after.elapsed_ms(), 1_000);
    }

    #[test]
    fn clock_before_epoch_saturates_to_zero() {
        let generator = SnowflakeGenerator::with_clock(1, ManualClock::at(0)).unwrap();
        let first = generator.generate();
        let second = generator.generate();
        assert_eq!(first.elapsed_ms(), 0);
        assert!(second > first);
    }

    #[test]
    fn unique_and_monotonic_across_threads() {
        const THREADS: usize = 8;
        const PER_THREAD: usize = 25_000;

        let generator = Arc::new(SnowflakeGenerator::new(42).unwrap());
        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let generator = Arc::clone(&generator);
                thread::spawn(move || {
                    (0..PER_THREAD)
                        .map(|_| generator.generate())
                        .collect::<Vec<_>>()
                })
            })
            .collect();

        let mut all = HashSet::with_capacity(THREADS * PER_THREAD);
        for handle in handles {
            let ids = handle.join().unwrap();
            assert!(
                ids.windows(2).all(|pair| pair[0] < pair[1]),
                "ids from one thread must be strictly increasing"
            );
            all.extend(ids);
        }
        assert_eq!(all.len(), THREADS * PER_THREAD);
    }
}
