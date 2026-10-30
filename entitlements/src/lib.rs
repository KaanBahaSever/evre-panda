//! Plan definitions and entitlement checks shared by server and clients.
//!
//! A guild's effective [`Entitlements`] are resolved in three steps
//! (architecture.md §12):
//!
//! 1. the base values of its plan ([`PlanId::entitlements`], §12.1),
//! 2. per-guild [`Overrides`] replace individual values (internal plan, promos),
//! 3. purchased [`AddOns`] are added on top.
//!
//! The server enforces limits with the `check_*` methods on [`Entitlements`];
//! clients use the same values to show locked options with an upsell.
//!
//! Units: sizes are bytes, using binary multiples ([`MIB`], [`GIB`]); the
//! "MB"/"GB" in §12.1 mean MiB/GiB. Bitrates are kbit/s.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod effective;
mod limit;
mod overrides;
mod plan;
mod stream;

pub use effective::Entitlements;
pub use limit::{Limit, LimitError, LimitKey, LimitValue, ParseLimitKeyError};
pub use overrides::{AddOn, AddOns, Overrides};
pub use plan::{Plan, PlanId};
pub use stream::{StreamLimit, StreamQuality, StreamResolution};

/// One mebibyte (2^20 bytes).
pub const MIB: u64 = 1024 * 1024;

/// One gibibyte (2^30 bytes).
pub const GIB: u64 = 1024 * MIB;

/// Highest bitrate Opus can encode, in kbit/s. Used where §12.1 says "256+".
pub const OPUS_MAX_BITRATE_KBPS: u32 = 510;
