//! Effective entitlements of a guild and the checks that enforce them.

use crate::limit::{Limit, LimitError, LimitKey, LimitValue};
use crate::overrides::{AddOn, AddOns, Overrides};
use crate::plan::PlanId;
use crate::stream::{StreamLimit, StreamQuality, StreamResolution};

/// The limits that apply to one guild.
///
/// Build it with [`Entitlements::resolve`] (or start from
/// [`PlanId::entitlements`]) and enforce it with the `check_*` methods. Count
/// checks take the total *after* the operation, e.g. `current + 1` for a join.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Entitlements {
    /// Members in the guild.
    pub max_members: Limit,
    /// Users connected to voice in the guild at the same time.
    pub max_concurrent_voice: Limit,
    /// Highest voice track bitrate a channel may be set to (kbit/s).
    pub max_voice_bitrate_kbps: u32,
    /// Highest music bitrate for bots (kbit/s); 0 when bots are not allowed.
    pub max_music_bitrate_kbps: u32,
    /// Customer-run bots in the guild.
    pub max_bots: Limit,
    /// Highest screen share qualities.
    pub max_stream: StreamLimit,
    /// Screen shares running in the guild at the same time.
    pub max_concurrent_streams: Limit,
    /// Size of a single uploaded file (bytes).
    pub max_file_size_bytes: Limit,
    /// Total stored attachment bytes for the guild.
    pub storage_quota_bytes: Limit,
    /// Custom emoji in the guild.
    pub custom_emoji_slots: Limit,
    /// How far back (days) the owner can see the audit log.
    pub audit_log_retention_days: Limit,
    /// Whether the guild may set a vanity invite.
    pub vanity_invite: bool,
    /// Highest quality of a phone camera companion's video (ADR-076);
    /// `720p30` on every plan for now.
    pub max_camera: StreamQuality,
}

impl Entitlements {
    /// Effective entitlements: plan base values, then `overrides` (which
    /// replace values), then `add_ons` (which add on top).
    #[must_use]
    pub fn resolve(plan: PlanId, overrides: &Overrides, add_ons: &AddOns) -> Self {
        plan.entitlements()
            .with_overrides(overrides)
            .with_add_ons(add_ons)
    }

    /// Replaces every value that `overrides` sets.
    #[must_use]
    pub fn with_overrides(mut self, overrides: &Overrides) -> Self {
        fn set<T: Clone>(target: &mut T, value: Option<&T>) {
            if let Some(value) = value {
                *target = value.clone();
            }
        }

        set(&mut self.max_members, overrides.max_members.as_ref());
        set(
            &mut self.max_concurrent_voice,
            overrides.max_concurrent_voice.as_ref(),
        );
        set(
            &mut self.max_voice_bitrate_kbps,
            overrides.max_voice_bitrate_kbps.as_ref(),
        );
        set(
            &mut self.max_music_bitrate_kbps,
            overrides.max_music_bitrate_kbps.as_ref(),
        );
        set(&mut self.max_bots, overrides.max_bots.as_ref());
        set(&mut self.max_stream, overrides.max_stream.as_ref());
        set(
            &mut self.max_concurrent_streams,
            overrides.max_concurrent_streams.as_ref(),
        );
        set(
            &mut self.max_file_size_bytes,
            overrides.max_file_size_bytes.as_ref(),
        );
        set(
            &mut self.storage_quota_bytes,
            overrides.storage_quota_bytes.as_ref(),
        );
        set(
            &mut self.custom_emoji_slots,
            overrides.custom_emoji_slots.as_ref(),
        );
        set(
            &mut self.audit_log_retention_days,
            overrides.audit_log_retention_days.as_ref(),
        );
        set(&mut self.vanity_invite, overrides.vanity_invite.as_ref());
        set(&mut self.max_camera, overrides.max_camera.as_ref());
        self
    }

    /// Adds purchased add-on packs. Unlimited values stay unlimited.
    #[must_use]
    pub fn with_add_ons(mut self, add_ons: &AddOns) -> Self {
        self.max_concurrent_voice = self
            .max_concurrent_voice
            .saturating_add(add_ons.total(AddOn::VoiceSlots));
        self.storage_quota_bytes = self
            .storage_quota_bytes
            .saturating_add(add_ons.total(AddOn::Storage));
        self.max_concurrent_streams = self
            .max_concurrent_streams
            .saturating_add(add_ons.total(AddOn::ConcurrentStream));
        self
    }

    /// Checks the member count after a join.
    pub fn check_member_count(&self, total: u64) -> Result<(), LimitError> {
        self.max_members.check(LimitKey::MaxMembers, total)
    }

    /// Checks the number of users in voice (guild-wide) after a join.
    pub fn check_concurrent_voice(&self, total: u64) -> Result<(), LimitError> {
        self.max_concurrent_voice
            .check(LimitKey::MaxConcurrentVoice, total)
    }

    /// Checks a voice bitrate (channel setting or track grant).
    pub fn check_voice_bitrate(&self, kbps: u32) -> Result<(), LimitError> {
        check_bitrate(
            LimitKey::MaxVoiceBitrateKbps,
            self.max_voice_bitrate_kbps,
            kbps,
        )
    }

    /// Checks a bot music bitrate.
    pub fn check_music_bitrate(&self, kbps: u32) -> Result<(), LimitError> {
        check_bitrate(
            LimitKey::MaxMusicBitrateKbps,
            self.max_music_bitrate_kbps,
            kbps,
        )
    }

    /// Checks the number of bots after adding one.
    pub fn check_bot_count(&self, total: u64) -> Result<(), LimitError> {
        self.max_bots.check(LimitKey::MaxBots, total)
    }

    /// Checks a stream permit request: quality first, then the number of
    /// concurrent streams in the guild including the new one.
    pub fn check_stream(
        &self,
        resolution: StreamResolution,
        fps: u32,
        concurrent_total: u64,
    ) -> Result<(), LimitError> {
        self.max_stream.check(resolution, fps)?;
        self.max_concurrent_streams
            .check(LimitKey::MaxConcurrentStreams, concurrent_total)
    }

    /// Checks the size of a single file (bytes).
    pub fn check_file_size(&self, bytes: u64) -> Result<(), LimitError> {
        self.max_file_size_bytes
            .check(LimitKey::MaxFileSizeBytes, bytes)
    }

    /// Checks whether `additional_bytes` fit next to `used_bytes` of storage.
    pub fn check_storage(&self, used_bytes: u64, additional_bytes: u64) -> Result<(), LimitError> {
        self.storage_quota_bytes.check(
            LimitKey::StorageQuotaBytes,
            used_bytes.saturating_add(additional_bytes),
        )
    }

    /// Checks the number of custom emoji after adding one.
    pub fn check_custom_emoji_count(&self, total: u64) -> Result<(), LimitError> {
        self.custom_emoji_slots
            .check(LimitKey::CustomEmojiSlots, total)
    }

    /// Checks whether an audit log entry `age_days` old is visible to the owner.
    pub fn check_audit_log_age(&self, age_days: u64) -> Result<(), LimitError> {
        self.audit_log_retention_days
            .check(LimitKey::AuditLogRetentionDays, age_days)
    }

    /// The camera companion's quality cap as its wire name, e.g. `"720p30"`
    /// (`max_camera_resolution`, ADR-076).
    #[must_use]
    pub fn camera_resolution(&self) -> String {
        format!("{}{}", self.max_camera.resolution, self.max_camera.fps)
    }

    /// Checks whether the guild may set a vanity invite.
    pub fn check_vanity_invite(&self) -> Result<(), LimitError> {
        if self.vanity_invite {
            Ok(())
        } else {
            Err(LimitError {
                key: LimitKey::VanityInvite,
                limit: LimitValue::Flag(false),
                requested: LimitValue::Flag(true),
            })
        }
    }
}

fn check_bitrate(key: LimitKey, max_kbps: u32, kbps: u32) -> Result<(), LimitError> {
    Limit::Max(u64::from(max_kbps)).check(key, u64::from(kbps))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::StreamQuality;
    use crate::{GIB, MIB, OPUS_MAX_BITRATE_KBPS};
    use StreamResolution::*;

    fn err(key: LimitKey, limit: u64, requested: u64) -> Result<(), LimitError> {
        Err(LimitError {
            key,
            limit: LimitValue::Number(limit),
            requested: LimitValue::Number(requested),
        })
    }

    /// Asserts that a `Limit::Max(max)` check passes at `max` and fails at
    /// `max + 1`, or that an unlimited check passes for `u64::MAX`.
    fn assert_boundary(limit: Limit, key: LimitKey, check: impl Fn(u64) -> Result<(), LimitError>) {
        match limit {
            Limit::Max(max) => {
                assert_eq!(check(max), Ok(()), "{key} at limit {max}");
                assert_eq!(check(max + 1), err(key, max, max + 1), "{key} above {max}");
            }
            Limit::Unlimited => assert_eq!(check(u64::MAX), Ok(()), "{key} unlimited"),
        }
    }

    /// One row of the §12.1 table.
    struct Row {
        plan: PlanId,
        members: Limit,
        voice: Limit,
        voice_kbps: u32,
        bots: Limit,
        music_kbps: u32,
        stream: StreamLimit,
        streams: Limit,
        file: Limit,
        storage: Limit,
        emoji: Limit,
        audit_days: Limit,
        vanity: bool,
    }

    fn table() -> Vec<Row> {
        use Limit::{Max, Unlimited};
        let q = StreamQuality::new;
        vec![
            Row {
                plan: PlanId::Free,
                members: Max(10),
                voice: Max(4),
                voice_kbps: 64,
                bots: Max(0),
                music_kbps: 0,
                stream: StreamLimit::new(q(P720, 30)),
                streams: Max(1),
                file: Max(8 * MIB),
                storage: Max(250 * MIB),
                emoji: Max(10),
                audit_days: Max(7),
                vanity: false,
            },
            Row {
                plan: PlanId::Standard,
                members: Max(150),
                voice: Max(25),
                voice_kbps: 96,
                bots: Max(3),
                music_kbps: 192,
                stream: StreamLimit::new(q(P1080, 60)),
                streams: Max(2),
                file: Max(25 * MIB),
                storage: Max(2 * GIB),
                emoji: Max(50),
                audit_days: Max(30),
                vanity: false,
            },
            Row {
                plan: PlanId::Pro,
                members: Max(500),
                voice: Max(75),
                voice_kbps: 128,
                bots: Max(10),
                music_kbps: 256,
                stream: StreamLimit::new(q(P1440, 60)),
                streams: Max(5),
                file: Max(50 * MIB),
                storage: Max(10 * GIB),
                emoji: Max(150),
                audit_days: Max(90),
                vanity: true,
            },
            Row {
                plan: PlanId::Community,
                members: Max(2_000),
                voice: Max(200),
                voice_kbps: 256,
                bots: Max(25),
                music_kbps: 256,
                stream: StreamLimit::new(q(P1440, 60)).or(q(P2160, 30)),
                streams: Max(10),
                file: Max(100 * MIB),
                storage: Max(25 * GIB),
                emoji: Max(300),
                audit_days: Max(365),
                vanity: true,
            },
            Row {
                plan: PlanId::Internal,
                members: Unlimited,
                voice: Unlimited,
                voice_kbps: OPUS_MAX_BITRATE_KBPS,
                bots: Unlimited,
                music_kbps: OPUS_MAX_BITRATE_KBPS,
                stream: StreamLimit::new(q(Source, 60)),
                streams: Unlimited,
                file: Max(10 * GIB),
                storage: Unlimited,
                emoji: Unlimited,
                audit_days: Unlimited,
                vanity: true,
            },
        ]
    }

    #[test]
    fn plans_match_the_architecture_table() {
        for row in table() {
            let e = row.plan.entitlements();
            let name = row.plan;
            assert_eq!(e.max_members, row.members, "{name} members");
            assert_eq!(e.max_concurrent_voice, row.voice, "{name} voice");
            assert_eq!(
                e.max_voice_bitrate_kbps, row.voice_kbps,
                "{name} voice kbps"
            );
            assert_eq!(e.max_bots, row.bots, "{name} bots");
            assert_eq!(e.max_music_bitrate_kbps, row.music_kbps, "{name} music");
            assert_eq!(e.max_stream, row.stream, "{name} stream");
            assert_eq!(e.max_concurrent_streams, row.streams, "{name} streams");
            assert_eq!(e.max_file_size_bytes, row.file, "{name} file size");
            assert_eq!(e.storage_quota_bytes, row.storage, "{name} storage");
            assert_eq!(e.custom_emoji_slots, row.emoji, "{name} emoji");
            assert_eq!(e.audit_log_retention_days, row.audit_days, "{name} audit");
            assert_eq!(e.vanity_invite, row.vanity, "{name} vanity");
        }
    }

    #[test]
    fn count_checks_enforce_every_plan_boundary() {
        for row in table() {
            let e = row.plan.entitlements();
            assert_boundary(row.members, LimitKey::MaxMembers, |n| {
                e.check_member_count(n)
            });
            assert_boundary(row.voice, LimitKey::MaxConcurrentVoice, |n| {
                e.check_concurrent_voice(n)
            });
            assert_boundary(row.bots, LimitKey::MaxBots, |n| e.check_bot_count(n));
            assert_boundary(row.file, LimitKey::MaxFileSizeBytes, |n| {
                e.check_file_size(n)
            });
            assert_boundary(row.storage, LimitKey::StorageQuotaBytes, |n| {
                e.check_storage(0, n)
            });
            assert_boundary(row.emoji, LimitKey::CustomEmojiSlots, |n| {
                e.check_custom_emoji_count(n)
            });
            assert_boundary(row.audit_days, LimitKey::AuditLogRetentionDays, |n| {
                e.check_audit_log_age(n)
            });
            assert_boundary(row.streams, LimitKey::MaxConcurrentStreams, |n| {
                e.check_stream(P720, 30, n)
            });
        }
    }

    #[test]
    fn bitrate_checks_enforce_every_plan_boundary() {
        for row in table() {
            let e = row.plan.entitlements();
            let voice = row.voice_kbps;
            assert_eq!(e.check_voice_bitrate(voice), Ok(()));
            assert_eq!(
                e.check_voice_bitrate(voice + 1),
                err(
                    LimitKey::MaxVoiceBitrateKbps,
                    voice.into(),
                    (voice + 1).into()
                )
            );
            let music = row.music_kbps;
            assert_eq!(e.check_music_bitrate(music), Ok(()));
            assert_eq!(
                e.check_music_bitrate(music + 1),
                err(
                    LimitKey::MaxMusicBitrateKbps,
                    music.into(),
                    (music + 1).into()
                )
            );
        }
    }

    #[test]
    fn free_plan_has_no_bots() {
        let free = PlanId::Free.entitlements();
        assert_eq!(free.check_bot_count(0), Ok(()));
        assert_eq!(free.check_bot_count(1), err(LimitKey::MaxBots, 0, 1));
        assert!(free.check_music_bitrate(1).is_err());
    }

    #[test]
    fn stream_checks_per_plan() {
        use LimitKey::{MaxStreamFps, MaxStreamResolution};
        // (plan, resolution, fps, expected failing key)
        let cases = [
            (PlanId::Free, P720, 30, None),
            (PlanId::Free, P720, 15, None),
            (PlanId::Free, P720, 60, Some(MaxStreamFps)),
            (PlanId::Free, P1080, 30, Some(MaxStreamResolution)),
            (PlanId::Standard, P1080, 60, None),
            (PlanId::Standard, P1440, 30, Some(MaxStreamResolution)),
            (PlanId::Pro, P1440, 60, None),
            (PlanId::Pro, P2160, 30, Some(MaxStreamResolution)),
            (PlanId::Community, P1440, 60, None),
            (PlanId::Community, P2160, 30, None),
            (PlanId::Community, P2160, 60, Some(MaxStreamFps)),
            (PlanId::Community, Source, 30, Some(MaxStreamResolution)),
            (PlanId::Internal, P2160, 60, None),
            (PlanId::Internal, Source, 60, None),
            (PlanId::Internal, Source, 120, Some(MaxStreamFps)),
        ];
        for (plan, resolution, fps, expected) in cases {
            let result = plan.entitlements().check_stream(resolution, fps, 1);
            assert_eq!(
                result.err().map(|e| e.key),
                expected,
                "{plan} {resolution}@{fps}"
            );
        }
    }

    #[test]
    fn stream_quality_is_checked_before_concurrency() {
        let free = PlanId::Free.entitlements();
        let error = free.check_stream(P1080, 30, 99).unwrap_err();
        assert_eq!(error.key, LimitKey::MaxStreamResolution);
        assert_eq!(
            free.check_stream(P720, 30, 2),
            err(LimitKey::MaxConcurrentStreams, 1, 2)
        );
    }

    #[test]
    fn storage_check_adds_used_and_additional_without_overflow() {
        let free = PlanId::Free.entitlements();
        assert_eq!(free.check_storage(200 * MIB, 50 * MIB), Ok(()));
        assert_eq!(
            free.check_storage(200 * MIB, 50 * MIB + 1),
            err(LimitKey::StorageQuotaBytes, 250 * MIB, 250 * MIB + 1)
        );
        assert_eq!(
            free.check_storage(u64::MAX, u64::MAX),
            err(LimitKey::StorageQuotaBytes, 250 * MIB, u64::MAX)
        );
        let internal = PlanId::Internal.entitlements();
        assert_eq!(internal.check_storage(u64::MAX, u64::MAX), Ok(()));
    }

    #[test]
    fn vanity_invite_check() {
        for row in table() {
            let result = row.plan.entitlements().check_vanity_invite();
            assert_eq!(result.is_ok(), row.vanity, "{}", row.plan);
        }
        assert_eq!(
            PlanId::Standard.entitlements().check_vanity_invite(),
            Err(LimitError {
                key: LimitKey::VanityInvite,
                limit: LimitValue::Flag(false),
                requested: LimitValue::Flag(true),
            })
        );
    }

    #[test]
    fn empty_overrides_and_add_ons_keep_plan_values() {
        for plan in PlanId::ALL {
            assert_eq!(
                Entitlements::resolve(plan, &Overrides::default(), &AddOns::default()),
                plan.entitlements()
            );
        }
    }

    #[test]
    fn overrides_replace_only_the_values_they_set() {
        let overrides = Overrides {
            max_members: Some(Limit::Max(50)),
            max_bots: Some(Limit::Unlimited),
            max_voice_bitrate_kbps: Some(128),
            vanity_invite: Some(true),
            max_stream: Some(StreamLimit::new(StreamQuality::new(P1080, 60))),
            ..Overrides::default()
        };
        let e = Entitlements::resolve(PlanId::Free, &overrides, &AddOns::default());
        let free = PlanId::Free.entitlements();

        assert_eq!(e.max_members, Limit::Max(50));
        assert_eq!(e.max_bots, Limit::Unlimited);
        assert_eq!(e.max_voice_bitrate_kbps, 128);
        assert!(e.vanity_invite);
        assert_eq!(e.check_stream(P1080, 60, 1), Ok(()));
        // Untouched values come from the plan.
        assert_eq!(e.max_concurrent_voice, free.max_concurrent_voice);
        assert_eq!(e.storage_quota_bytes, free.storage_quota_bytes);
        assert_eq!(e.max_file_size_bytes, free.max_file_size_bytes);
        assert_eq!(e.audit_log_retention_days, free.audit_log_retention_days);
    }

    #[test]
    fn overrides_can_lower_limits() {
        let overrides = Overrides {
            max_file_size_bytes: Some(Limit::Max(MIB)),
            max_concurrent_streams: Some(Limit::Max(0)),
            vanity_invite: Some(false),
            ..Overrides::default()
        };
        let e = Entitlements::resolve(PlanId::Internal, &overrides, &AddOns::default());
        assert_eq!(
            e.check_file_size(MIB + 1),
            err(LimitKey::MaxFileSizeBytes, MIB, MIB + 1)
        );
        assert_eq!(
            e.check_stream(P720, 30, 1),
            err(LimitKey::MaxConcurrentStreams, 0, 1)
        );
        assert!(e.check_vanity_invite().is_err());
        // Everything else is still unlimited.
        assert_eq!(e.check_member_count(u64::MAX), Ok(()));
    }

    #[test]
    fn add_ons_stack_on_plan_values() {
        let add_ons = AddOns {
            voice_slot_packs: 2,
            storage_packs: 1,
            stream_packs: 3,
        };
        let e = Entitlements::resolve(PlanId::Standard, &Overrides::default(), &add_ons);
        assert_eq!(e.max_concurrent_voice, Limit::Max(25 + 20));
        assert_eq!(e.storage_quota_bytes, Limit::Max(2 * GIB + 10 * GIB));
        assert_eq!(e.max_concurrent_streams, Limit::Max(2 + 3));
        assert_eq!(e.check_concurrent_voice(45), Ok(()));
        assert!(e.check_concurrent_voice(46).is_err());
        assert_eq!(e.check_stream(P1080, 60, 5), Ok(()));
        assert!(e.check_stream(P1080, 60, 6).is_err());
        // Add-ons do not touch unrelated limits.
        assert_eq!(e.max_members, Limit::Max(150));
    }

    #[test]
    fn add_ons_stack_on_top_of_overrides() {
        let overrides = Overrides {
            max_concurrent_voice: Some(Limit::Max(50)),
            ..Overrides::default()
        };
        let add_ons = AddOns {
            voice_slot_packs: 1,
            ..AddOns::default()
        };
        let e = Entitlements::resolve(PlanId::Pro, &overrides, &add_ons);
        assert_eq!(e.max_concurrent_voice, Limit::Max(60));
    }

    #[test]
    fn add_ons_keep_unlimited_and_saturate() {
        let add_ons = AddOns {
            voice_slot_packs: u32::MAX,
            storage_packs: u32::MAX,
            stream_packs: u32::MAX,
        };
        let internal = Entitlements::resolve(PlanId::Internal, &Overrides::default(), &add_ons);
        assert_eq!(internal.max_concurrent_voice, Limit::Unlimited);
        assert_eq!(internal.storage_quota_bytes, Limit::Unlimited);
        assert_eq!(internal.max_concurrent_streams, Limit::Unlimited);

        let overrides = Overrides {
            storage_quota_bytes: Some(Limit::Max(u64::MAX - 1)),
            ..Overrides::default()
        };
        let free = Entitlements::resolve(PlanId::Free, &overrides, &add_ons);
        assert_eq!(free.storage_quota_bytes, Limit::Max(u64::MAX));
        assert_eq!(
            free.max_concurrent_voice,
            Limit::Max(4 + 10 * u64::from(u32::MAX))
        );
    }
}
