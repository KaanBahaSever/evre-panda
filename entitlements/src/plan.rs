//! The plan catalog (architecture.md §12.1).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::effective::Entitlements;
use crate::limit::Limit;
use crate::stream::{StreamLimit, StreamQuality, StreamResolution};
use crate::{GIB, MIB, OPUS_MAX_BITRATE_KBPS};

/// Identifies a guild plan. Serialized as `"free"`, `"standard"`, `"pro"`,
/// `"community"` or `"internal"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlanId {
    /// Free tier (tentative, §19 open question 2).
    Free,
    /// Standard paid plan.
    Standard,
    /// Pro paid plan.
    Pro,
    /// Community paid plan (largest public tier).
    Community,
    /// Our own guilds: effectively unlimited, never sold.
    Internal,
}

impl PlanId {
    /// Every plan, cheapest first.
    pub const ALL: [Self; 5] = [
        Self::Free,
        Self::Standard,
        Self::Pro,
        Self::Community,
        Self::Internal,
    ];

    /// The wire name of the plan.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Standard => "standard",
            Self::Pro => "pro",
            Self::Community => "community",
            Self::Internal => "internal",
        }
    }

    /// The full plan definition.
    #[must_use]
    pub fn plan(self) -> Plan {
        Plan {
            id: self,
            tentative: matches!(self, Self::Free),
            reference_price_usd_cents: match self {
                Self::Free => Some(0),
                Self::Standard => Some(400),
                Self::Pro => Some(1_000),
                Self::Community => Some(2_500),
                Self::Internal => None,
            },
            entitlements: self.entitlements(),
        }
    }

    /// The plan's base entitlements, before overrides and add-ons.
    #[must_use]
    pub fn entitlements(self) -> Entitlements {
        use StreamResolution::{P720, P1080, P1440, P2160, Source};
        // The phone camera companion (ADR-076): the same on every plan for
        // now.
        const CAMERA: StreamQuality = StreamQuality::new(P720, 30);

        match self {
            Self::Free => Entitlements {
                max_members: Limit::Max(10),
                max_concurrent_voice: Limit::Max(4),
                max_voice_bitrate_kbps: 64,
                // No bots on Free, so no music bitrate either.
                max_music_bitrate_kbps: 0,
                max_bots: Limit::Max(0),
                max_stream: StreamLimit::new(StreamQuality::new(P720, 30)),
                max_concurrent_streams: Limit::Max(1),
                max_file_size_bytes: Limit::Max(8 * MIB),
                storage_quota_bytes: Limit::Max(250 * MIB),
                custom_emoji_slots: Limit::Max(10),
                audit_log_retention_days: Limit::Max(7),
                vanity_invite: false,
                max_camera: CAMERA,
            },
            Self::Standard => Entitlements {
                max_members: Limit::Max(150),
                max_concurrent_voice: Limit::Max(25),
                max_voice_bitrate_kbps: 96,
                max_music_bitrate_kbps: 192,
                max_bots: Limit::Max(3),
                max_stream: StreamLimit::new(StreamQuality::new(P1080, 60)),
                max_concurrent_streams: Limit::Max(2),
                max_file_size_bytes: Limit::Max(25 * MIB),
                storage_quota_bytes: Limit::Max(2 * GIB),
                custom_emoji_slots: Limit::Max(50),
                audit_log_retention_days: Limit::Max(30),
                vanity_invite: false,
                max_camera: CAMERA,
            },
            Self::Pro => Entitlements {
                max_members: Limit::Max(500),
                max_concurrent_voice: Limit::Max(75),
                max_voice_bitrate_kbps: 128,
                max_music_bitrate_kbps: 256,
                max_bots: Limit::Max(10),
                max_stream: StreamLimit::new(StreamQuality::new(P1440, 60)),
                max_concurrent_streams: Limit::Max(5),
                max_file_size_bytes: Limit::Max(50 * MIB),
                storage_quota_bytes: Limit::Max(10 * GIB),
                custom_emoji_slots: Limit::Max(150),
                audit_log_retention_days: Limit::Max(90),
                vanity_invite: true,
                max_camera: CAMERA,
            },
            Self::Community => Entitlements {
                max_members: Limit::Max(2_000),
                max_concurrent_voice: Limit::Max(200),
                max_voice_bitrate_kbps: 256,
                max_music_bitrate_kbps: 256,
                max_bots: Limit::Max(25),
                max_stream: StreamLimit::new(StreamQuality::new(P1440, 60))
                    .or(StreamQuality::new(P2160, 30)),
                max_concurrent_streams: Limit::Max(10),
                max_file_size_bytes: Limit::Max(100 * MIB),
                storage_quota_bytes: Limit::Max(25 * GIB),
                custom_emoji_slots: Limit::Max(300),
                audit_log_retention_days: Limit::Max(365),
                vanity_invite: true,
                max_camera: CAMERA,
            },
            Self::Internal => Entitlements {
                max_members: Limit::Unlimited,
                max_concurrent_voice: Limit::Unlimited,
                // "256+" in §12.1: allow up to what Opus can encode.
                max_voice_bitrate_kbps: OPUS_MAX_BITRATE_KBPS,
                max_music_bitrate_kbps: OPUS_MAX_BITRATE_KBPS,
                max_bots: Limit::Unlimited,
                // "Source / 4K60": native resolution (4K included) up to 60 fps.
                max_stream: StreamLimit::new(StreamQuality::new(Source, 60)),
                max_concurrent_streams: Limit::Unlimited,
                // "Very large (e.g. 10 GB), disk-bound".
                max_file_size_bytes: Limit::Max(10 * GIB),
                // "Disk-bound": no plan limit, only physical disk space.
                storage_quota_bytes: Limit::Unlimited,
                custom_emoji_slots: Limit::Unlimited,
                audit_log_retention_days: Limit::Unlimited,
                vanity_invite: true,
                max_camera: CAMERA,
            },
        }
    }
}

impl fmt::Display for PlanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A plan from the catalog with its metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// Which plan this is.
    pub id: PlanId,
    /// Whether the plan's limits are still a draft (the Free tier).
    pub tentative: bool,
    /// Monthly reference price in US cents (sold in TRY, repriced
    /// periodically); `None` for plans that are not sold.
    pub reference_price_usd_cents: Option<u32>,
    /// Base entitlements.
    pub entitlements: Entitlements,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_ids_serialize_lowercase() {
        for plan in PlanId::ALL {
            let json = format!("\"{}\"", plan.as_str());
            assert_eq!(serde_json::to_string(&plan).unwrap(), json);
            assert_eq!(serde_json::from_str::<PlanId>(&json).unwrap(), plan);
            assert_eq!(plan.to_string(), plan.as_str());
        }
        assert_eq!(PlanId::Community.as_str(), "community");
    }

    #[test]
    fn plan_metadata() {
        assert!(PlanId::Free.plan().tentative);
        assert!(!PlanId::Pro.plan().tentative);
        let prices: Vec<_> = PlanId::ALL
            .iter()
            .map(|p| p.plan().reference_price_usd_cents)
            .collect();
        assert_eq!(prices, [Some(0), Some(400), Some(1_000), Some(2_500), None]);
        assert_eq!(PlanId::Pro.plan().entitlements, PlanId::Pro.entitlements());
        assert_eq!(PlanId::Pro.plan().id, PlanId::Pro);
    }

    #[test]
    fn every_plan_allows_a_720p30_camera() {
        for plan in PlanId::ALL {
            assert_eq!(plan.entitlements().camera_resolution(), "720p30");
        }
    }
}
