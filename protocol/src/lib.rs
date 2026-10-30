//! Shared wire types for Era: IDs, models, REST bodies, gateway messages, API
//! errors and permission bits (architecture.md §9, §10).
//!
//! This crate is the contract between the server, clients, the bot SDK and
//! bots. It holds plain data with serde support and no I/O.
//!
//! # Wire conventions
//!
//! - JSON field names are `camelCase`; enum values are `camelCase` strings.
//! - [`Snowflake`] IDs and [`Permissions`] are **decimal strings**, because
//!   JavaScript numbers lose precision above 2^53. Decoders reject numbers.
//! - Timestamps ([`Timestamp`]) are Unix epoch milliseconds (integers).
//! - `Option` fields serialize as `null` and may be omitted on input.
//!
//! # Compatibility
//!
//! - Fields are only ever added, never removed or retyped; decoders ignore
//!   unknown fields.
//! - Unknown dispatch event types decode to [`Event::Unknown`] and unknown
//!   channel kinds to [`ChannelKind::Unknown`], so older clients keep working
//!   against newer servers.
//! - Any other incompatible change, including a new gateway opcode, requires
//!   bumping [`GATEWAY_VERSION`].

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod deeplink;
pub mod error;
pub mod gateway;
pub mod locale;
pub mod model;
pub mod permissions;
pub mod rest;
mod serde_util;
pub mod snowflake;
pub mod timestamp;

pub use entitlements::{LimitKey, PlanId};
pub use error::{ApiError, FieldError};
pub use gateway::{
    ChannelDelete, ClientInfo, DmDelete, Event, GATEWAY_VERSION, GatewayMessage, GuildDelete,
    GuildMemberAdd, GuildMemberRemove, GuildRoleDelete, JoinRequestCount, JoinRequestUpdate,
    MessageDelete, Nudge, PresenceUpdate, RelationshipRemove, Token, TypingStart,
};
pub use locale::Locale;
pub use model::{
    Attachment, AudioProfile, Channel, ChannelKind, Device, DeviceForm, DmChannel, Guild,
    GuildAccess, GuildCategory, Invite, Member, Message, OverwriteTarget, PermissionOverwrite,
    Platform, PresenceStatus, Relationship, RelationshipKind, Role, StatusSetting, User,
    VoiceSettings, VoiceState,
};
pub use model::{CAMERA_IDENTITY_SUFFIX, CameraSource, VoiceDevice};
pub use permissions::Permissions;
pub use snowflake::{Snowflake, SnowflakeGenerator};
pub use timestamp::Timestamp;
