//! Numeric limits, entitlement keys and limit errors.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::stream::StreamResolution;

/// An upper bound that is either a concrete maximum or explicitly unlimited.
///
/// Ordering treats [`Limit::Unlimited`] as larger than any [`Limit::Max`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Limit {
    /// At most this many (inclusive).
    Max(u64),
    /// No limit.
    Unlimited,
}

impl Limit {
    /// Whether `value` is within the limit.
    #[must_use]
    pub const fn allows(self, value: u64) -> bool {
        match self {
            Self::Max(max) => value <= max,
            Self::Unlimited => true,
        }
    }

    /// The concrete maximum, or `None` when unlimited.
    #[must_use]
    pub const fn max(self) -> Option<u64> {
        match self {
            Self::Max(max) => Some(max),
            Self::Unlimited => None,
        }
    }

    /// Whether this is [`Limit::Unlimited`].
    #[must_use]
    pub const fn is_unlimited(self) -> bool {
        matches!(self, Self::Unlimited)
    }

    /// Raises a concrete maximum by `extra` (saturating). Unlimited stays unlimited.
    #[must_use]
    pub const fn saturating_add(self, extra: u64) -> Self {
        match self {
            Self::Max(max) => Self::Max(max.saturating_add(extra)),
            Self::Unlimited => Self::Unlimited,
        }
    }

    /// Checks `requested` against the limit, reporting violations under `key`.
    pub fn check(self, key: LimitKey, requested: u64) -> Result<(), LimitError> {
        match self {
            Self::Max(max) if requested > max => Err(LimitError {
                key,
                limit: LimitValue::Number(max),
                requested: LimitValue::Number(requested),
            }),
            _ => Ok(()),
        }
    }
}

impl fmt::Display for Limit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Max(max) => write!(f, "{max}"),
            Self::Unlimited => f.write_str("unlimited"),
        }
    }
}

/// Stable identifier of an entitlement (architecture.md §12).
///
/// The string form (see [`LimitKey::as_str`]) is part of the API contract: it
/// appears in `PlanLimitReached` errors and in override records. It is
/// `snake_case` to match the documented key names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitKey {
    /// Members in the guild.
    MaxMembers,
    /// Users connected to voice in the guild at the same time.
    MaxConcurrentVoice,
    /// Voice track bitrate (kbit/s).
    MaxVoiceBitrateKbps,
    /// Bot music track bitrate (kbit/s).
    MaxMusicBitrateKbps,
    /// Screen share resolution.
    MaxStreamResolution,
    /// Screen share frame rate.
    MaxStreamFps,
    /// Screen shares running in the guild at the same time.
    MaxConcurrentStreams,
    /// Size of a single uploaded file (bytes).
    MaxFileSizeBytes,
    /// Total stored attachment bytes for the guild.
    StorageQuotaBytes,
    /// Customer-run bots in the guild.
    MaxBots,
    /// Custom emoji in the guild.
    CustomEmojiSlots,
    /// How far back (days) the owner can see the audit log.
    AuditLogRetentionDays,
    /// Custom vanity invite link.
    VanityInvite,
    /// Phone camera companion resolution and frame rate (ADR-076).
    MaxCameraResolution,
}

impl LimitKey {
    /// Every key, in declaration order.
    pub const ALL: [Self; 14] = [
        Self::MaxMembers,
        Self::MaxConcurrentVoice,
        Self::MaxVoiceBitrateKbps,
        Self::MaxMusicBitrateKbps,
        Self::MaxStreamResolution,
        Self::MaxStreamFps,
        Self::MaxConcurrentStreams,
        Self::MaxFileSizeBytes,
        Self::StorageQuotaBytes,
        Self::MaxBots,
        Self::CustomEmojiSlots,
        Self::AuditLogRetentionDays,
        Self::VanityInvite,
        Self::MaxCameraResolution,
    ];

    /// The stable `snake_case` name of the key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MaxMembers => "max_members",
            Self::MaxConcurrentVoice => "max_concurrent_voice",
            Self::MaxVoiceBitrateKbps => "max_voice_bitrate_kbps",
            Self::MaxMusicBitrateKbps => "max_music_bitrate_kbps",
            Self::MaxStreamResolution => "max_stream_resolution",
            Self::MaxStreamFps => "max_stream_fps",
            Self::MaxConcurrentStreams => "max_concurrent_streams",
            Self::MaxFileSizeBytes => "max_file_size_bytes",
            Self::StorageQuotaBytes => "storage_quota_bytes",
            Self::MaxBots => "max_bots",
            Self::CustomEmojiSlots => "custom_emoji_slots",
            Self::AuditLogRetentionDays => "audit_log_retention_days",
            Self::VanityInvite => "vanity_invite",
            Self::MaxCameraResolution => "max_camera_resolution",
        }
    }
}

impl fmt::Display for LimitKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Returned when parsing an unknown [`LimitKey`] name.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown entitlement key `{0}`")]
pub struct ParseLimitKeyError(pub String);

impl FromStr for LimitKey {
    type Err = ParseLimitKeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|key| key.as_str() == s)
            .ok_or_else(|| ParseLimitKeyError(s.to_owned()))
    }
}

/// A limit or requested amount, as reported in a [`LimitError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LimitValue {
    /// A count, size (bytes), bitrate (kbit/s), frame rate or number of days.
    Number(u64),
    /// A screen share resolution.
    Resolution(StreamResolution),
    /// A feature switch.
    Flag(bool),
}

impl fmt::Display for LimitValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(n) => write!(f, "{n}"),
            Self::Resolution(resolution) => write!(f, "{resolution}"),
            Self::Flag(enabled) => f.write_str(if *enabled { "enabled" } else { "disabled" }),
        }
    }
}

/// A request exceeded the guild's entitlements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("plan limit `{key}` reached: requested {requested}, allowed {limit}")]
pub struct LimitError {
    /// Which entitlement was exceeded.
    pub key: LimitKey,
    /// The effective limit.
    pub limit: LimitValue,
    /// What was requested (the resulting total for counts).
    pub requested: LimitValue,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_allows_inclusive_maximum() {
        assert!(Limit::Max(10).allows(10));
        assert!(!Limit::Max(10).allows(11));
        assert!(Limit::Max(0).allows(0));
        assert!(!Limit::Max(0).allows(1));
        assert!(Limit::Unlimited.allows(u64::MAX));
    }

    #[test]
    fn limit_check_reports_key_limit_and_request() {
        assert_eq!(Limit::Max(5).check(LimitKey::MaxBots, 5), Ok(()));
        assert_eq!(
            Limit::Max(5).check(LimitKey::MaxBots, 6),
            Err(LimitError {
                key: LimitKey::MaxBots,
                limit: LimitValue::Number(5),
                requested: LimitValue::Number(6),
            })
        );
        assert_eq!(Limit::Unlimited.check(LimitKey::MaxBots, u64::MAX), Ok(()));
    }

    #[test]
    fn limit_saturating_add_keeps_unlimited() {
        assert_eq!(Limit::Max(4).saturating_add(10), Limit::Max(14));
        assert_eq!(Limit::Max(u64::MAX).saturating_add(1), Limit::Max(u64::MAX));
        assert_eq!(Limit::Unlimited.saturating_add(10), Limit::Unlimited);
    }

    #[test]
    fn limit_ordering_puts_unlimited_last() {
        assert!(Limit::Max(1) < Limit::Max(2));
        assert!(Limit::Max(u64::MAX) < Limit::Unlimited);
        assert_eq!(Limit::Max(3).max(), Some(3));
        assert_eq!(Limit::Unlimited.max(), None);
        assert!(Limit::Unlimited.is_unlimited());
    }

    #[test]
    fn limit_display() {
        assert_eq!(Limit::Max(25).to_string(), "25");
        assert_eq!(Limit::Unlimited.to_string(), "unlimited");
    }

    #[test]
    fn limit_key_names_are_stable_and_round_trip() {
        let expected = [
            "max_members",
            "max_concurrent_voice",
            "max_voice_bitrate_kbps",
            "max_music_bitrate_kbps",
            "max_stream_resolution",
            "max_stream_fps",
            "max_concurrent_streams",
            "max_file_size_bytes",
            "storage_quota_bytes",
            "max_bots",
            "custom_emoji_slots",
            "audit_log_retention_days",
            "vanity_invite",
            "max_camera_resolution",
        ];
        for (key, name) in LimitKey::ALL.into_iter().zip(expected) {
            assert_eq!(key.as_str(), name);
            assert_eq!(key.to_string(), name);
            assert_eq!(name.parse::<LimitKey>(), Ok(key));
            assert_eq!(serde_json::to_string(&key).unwrap(), format!("\"{name}\""));
            assert_eq!(
                serde_json::from_str::<LimitKey>(&format!("\"{name}\"")).unwrap(),
                key
            );
        }
        assert_eq!(
            "max_everything".parse::<LimitKey>(),
            Err(ParseLimitKeyError("max_everything".into()))
        );
    }

    #[test]
    fn limit_error_display() {
        let err = LimitError {
            key: LimitKey::MaxStreamResolution,
            limit: LimitValue::Resolution(StreamResolution::P1080),
            requested: LimitValue::Resolution(StreamResolution::P1440),
        };
        assert_eq!(
            err.to_string(),
            "plan limit `max_stream_resolution` reached: requested 1440p, allowed 1080p"
        );
        assert_eq!(LimitValue::Flag(false).to_string(), "disabled");
    }
}
