//! Per-guild overrides and purchasable add-ons.

use crate::GIB;
use crate::limit::Limit;
use crate::stream::{StreamLimit, StreamQuality};

/// Per-guild replacements for plan values (internal plan tweaks, promos).
///
/// Every `Some` field replaces the plan's value; `None` keeps it. Overrides
/// are applied before [`AddOns`], so purchased add-ons always stack on top.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overrides {
    /// Replaces [`Entitlements::max_members`](crate::Entitlements::max_members).
    pub max_members: Option<Limit>,
    /// Replaces [`Entitlements::max_concurrent_voice`](crate::Entitlements::max_concurrent_voice).
    pub max_concurrent_voice: Option<Limit>,
    /// Replaces [`Entitlements::max_voice_bitrate_kbps`](crate::Entitlements::max_voice_bitrate_kbps).
    pub max_voice_bitrate_kbps: Option<u32>,
    /// Replaces [`Entitlements::max_music_bitrate_kbps`](crate::Entitlements::max_music_bitrate_kbps).
    pub max_music_bitrate_kbps: Option<u32>,
    /// Replaces [`Entitlements::max_bots`](crate::Entitlements::max_bots).
    pub max_bots: Option<Limit>,
    /// Replaces [`Entitlements::max_stream`](crate::Entitlements::max_stream).
    pub max_stream: Option<StreamLimit>,
    /// Replaces [`Entitlements::max_concurrent_streams`](crate::Entitlements::max_concurrent_streams).
    pub max_concurrent_streams: Option<Limit>,
    /// Replaces [`Entitlements::max_file_size_bytes`](crate::Entitlements::max_file_size_bytes).
    pub max_file_size_bytes: Option<Limit>,
    /// Replaces [`Entitlements::storage_quota_bytes`](crate::Entitlements::storage_quota_bytes).
    pub storage_quota_bytes: Option<Limit>,
    /// Replaces [`Entitlements::custom_emoji_slots`](crate::Entitlements::custom_emoji_slots).
    pub custom_emoji_slots: Option<Limit>,
    /// Replaces [`Entitlements::audit_log_retention_days`](crate::Entitlements::audit_log_retention_days).
    pub audit_log_retention_days: Option<Limit>,
    /// Replaces [`Entitlements::vanity_invite`](crate::Entitlements::vanity_invite).
    pub vanity_invite: Option<bool>,
    /// Replaces [`Entitlements::max_camera`](crate::Entitlements::max_camera).
    pub max_camera: Option<StreamQuality>,
}

/// A purchasable add-on pack (§12.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddOn {
    /// +10 concurrent voice slots.
    VoiceSlots,
    /// +10 GiB storage.
    Storage,
    /// +1 concurrent stream.
    ConcurrentStream,
}

impl AddOn {
    /// What one pack adds, in the unit of the affected limit (users, bytes,
    /// streams).
    #[must_use]
    pub const fn amount_per_pack(self) -> u64 {
        match self {
            Self::VoiceSlots => 10,
            Self::Storage => 10 * GIB,
            Self::ConcurrentStream => 1,
        }
    }

    /// Monthly reference price per pack in US cents.
    #[must_use]
    pub const fn reference_price_usd_cents(self) -> u32 {
        match self {
            Self::VoiceSlots | Self::Storage => 100,
            Self::ConcurrentStream => 150,
        }
    }
}

/// Add-on packs a guild has purchased. Each count is a number of packs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct AddOns {
    /// [`AddOn::VoiceSlots`] packs.
    pub voice_slot_packs: u32,
    /// [`AddOn::Storage`] packs.
    pub storage_packs: u32,
    /// [`AddOn::ConcurrentStream`] packs.
    pub stream_packs: u32,
}

impl AddOns {
    /// Number of packs purchased of `add_on`.
    #[must_use]
    pub const fn packs(&self, add_on: AddOn) -> u32 {
        match add_on {
            AddOn::VoiceSlots => self.voice_slot_packs,
            AddOn::Storage => self.storage_packs,
            AddOn::ConcurrentStream => self.stream_packs,
        }
    }

    /// Total amount added by the purchased packs of `add_on` (saturating).
    #[must_use]
    pub const fn total(&self, add_on: AddOn) -> u64 {
        add_on
            .amount_per_pack()
            .saturating_mul(self.packs(add_on) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_on_amounts_and_prices() {
        assert_eq!(AddOn::VoiceSlots.amount_per_pack(), 10);
        assert_eq!(AddOn::Storage.amount_per_pack(), 10 * GIB);
        assert_eq!(AddOn::ConcurrentStream.amount_per_pack(), 1);
        assert_eq!(AddOn::VoiceSlots.reference_price_usd_cents(), 100);
        assert_eq!(AddOn::Storage.reference_price_usd_cents(), 100);
        assert_eq!(AddOn::ConcurrentStream.reference_price_usd_cents(), 150);
    }

    #[test]
    fn add_on_totals() {
        let add_ons = AddOns {
            voice_slot_packs: 3,
            storage_packs: 2,
            stream_packs: 1,
        };
        assert_eq!(add_ons.total(AddOn::VoiceSlots), 30);
        assert_eq!(add_ons.total(AddOn::Storage), 20 * GIB);
        assert_eq!(add_ons.total(AddOn::ConcurrentStream), 1);
        assert_eq!(AddOns::default().total(AddOn::Storage), 0);
    }
}
