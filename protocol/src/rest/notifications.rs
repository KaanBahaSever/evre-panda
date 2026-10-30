//! Notification settings (`docs/contracts/notifications.md` §1–§2): what
//! notifies, stored on the server and the same on every device.
//!
//! - `GET /v1/users/@me/notification-settings` → [`NotificationSettings`].
//! - `PATCH /v1/users/@me/notification-settings` with a
//!   [`NotificationSettingsPatch`] (the same shape; absent fields stay,
//!   `null` deletes an override) → the new [`NotificationSettings`]; every
//!   session of the user gets it as `notificationSettingsUpdate`.
//!
//! The rules that depend only on the settings live here
//! ([`NotificationSettings::decide`]), so the server's pushes and every
//! client's local notifications decide alike. The rules that need the
//! client's view (own messages, blocked authors, the channel in view) are in
//! `client_core::notify`; the server applies its own (visibility, blocks).

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::serde_util::double_option;
use crate::snowflake::Snowflake;
use crate::timestamp::Timestamp;

/// Guilds with more members than this default to [`NotificationLevel::Mentions`],
/// smaller ones to [`NotificationLevel::All`] (contract §1).
pub const LARGE_GUILD_MEMBERS: usize = 100;

/// Most guild entries one user may store.
pub const MAX_GUILD_ENTRIES: usize = 500;
/// Most category plus channel entries per guild.
pub const MAX_SCOPE_ENTRIES: usize = 500;
/// Most DM entries.
pub const MAX_DM_ENTRIES: usize = 1000;

/// What a guild, category or channel notifies of. The narrowest scope that
/// is not [`Self::Inherit`] wins; a guild that inherits uses
/// [`default_guild_level`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotificationLevel {
    /// Every message.
    All,
    /// Only mentions of me and replies to me.
    Mentions,
    /// Nothing, mentions included.
    None,
    /// The wider scope decides (also any level a newer peer sends).
    #[default]
    #[serde(other)]
    Inherit,
}

/// The level of a guild nobody set: `all` up to [`LARGE_GUILD_MEMBERS`]
/// members, `mentions` above.
#[must_use]
pub const fn default_guild_level(member_count: usize) -> NotificationLevel {
    if member_count > LARGE_GUILD_MEMBERS {
        NotificationLevel::Mentions
    } else {
        NotificationLevel::All
    }
}

/// When a mute ends: RFC 3339 on the wire (`"2026-10-28T08:00:00Z"`, any
/// offset accepted, always written in UTC), Unix milliseconds inside.
/// "Ben açana kadar" is [`Self::FOREVER`] (year 9999).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MuteUntil(Timestamp);

/// The mute presets the UI offers (contract §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MutePreset {
    /// 15 dk.
    Minutes15,
    /// 1 saat.
    Hour1,
    /// Yarına kadar: the next 08:00 local time.
    UntilTomorrow,
    /// Ben açana kadar.
    Forever,
}

impl MuteUntil {
    /// 9999-12-31T23:59:59Z: until the user unmutes.
    pub const FOREVER: Self = Self(Timestamp::from_unix_millis(253_402_300_799_000));

    /// A mute ending at `at`.
    #[must_use]
    pub const fn at(at: Timestamp) -> Self {
        Self(at)
    }

    /// When it ends.
    #[must_use]
    pub const fn timestamp(self) -> Timestamp {
        self.0
    }

    /// Whether it still mutes at `now`.
    #[must_use]
    pub fn is_active(self, now: Timestamp) -> bool {
        self.0 > now
    }

    /// The end of `preset` from `now`, for a user whose local time is UTC
    /// plus `utc_offset_minutes` ("Yarına kadar" is the next 08:00 there).
    #[must_use]
    pub fn preset(preset: MutePreset, now: Timestamp, utc_offset_minutes: i32) -> Self {
        const MINUTE: u64 = 60_000;
        const DAY: i64 = 86_400_000;
        let now_ms = now.unix_millis();
        match preset {
            MutePreset::Minutes15 => Self(Timestamp::from_unix_millis(now_ms + 15 * MINUTE)),
            MutePreset::Hour1 => Self(Timestamp::from_unix_millis(now_ms + 60 * MINUTE)),
            MutePreset::Forever => Self::FOREVER,
            MutePreset::UntilTomorrow => {
                let offset = i64::from(utc_offset_minutes) * 60_000;
                let local = i64::try_from(now_ms).unwrap_or(i64::MAX) + offset;
                let today = local.div_euclid(DAY) * DAY;
                let eight = today + 8 * 3_600_000;
                let next = if eight > local { eight } else { eight + DAY };
                Self(Timestamp::from_unix_millis(
                    u64::try_from(next - offset).unwrap_or(0),
                ))
            }
        }
    }

    /// RFC 3339 in UTC, milliseconds only when there are any.
    #[must_use]
    pub fn to_rfc3339(self) -> String {
        let ms = self.0.unix_millis();
        let secs = ms / 1000;
        let (year, month, day) = civil_from_days(i64::try_from(secs / 86_400).unwrap_or(0));
        let rest = secs % 86_400;
        let (hour, minute, second) = (rest / 3600, rest % 3600 / 60, rest % 60);
        let millis = ms % 1000;
        if millis == 0 {
            format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
        } else {
            format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
        }
    }

    /// Parses RFC 3339 (`YYYY-MM-DDTHH:MM:SS[.frac](Z|±HH:MM)`). Times
    /// before 1970 become 1970 (an ended mute).
    #[must_use]
    pub fn parse_rfc3339(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.len() < 20 || bytes.len() > 40 {
            return None;
        }
        let number = |range: std::ops::Range<usize>| -> Option<i64> {
            let part = text.get(range)?;
            if !part.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            part.parse().ok()
        };
        let year = number(0..4)?;
        let month = number(5..7)?;
        let day = number(8..10)?;
        let hour = number(11..13)?;
        let minute = number(14..16)?;
        let second = number(17..19)?;
        if bytes[4] != b'-'
            || bytes[7] != b'-'
            || !matches!(bytes[10], b'T' | b't' | b' ')
            || bytes[13] != b':'
            || bytes[16] != b':'
        {
            return None;
        }
        if !(1..=12).contains(&month)
            || day < 1
            || day > days_in_month(year, month)
            || hour > 23
            || minute > 59
            || second > 60
        {
            return None;
        }
        let mut index = 19;
        let mut millis = 0;
        if bytes.get(index) == Some(&b'.') {
            index += 1;
            let start = index;
            while bytes.get(index).is_some_and(u8::is_ascii_digit) {
                index += 1;
            }
            if index == start {
                return None;
            }
            let digits = &text[start..index.min(start + 3)];
            millis = digits.parse::<i64>().ok()? * 10_i64.pow(3 - digits.len() as u32);
        }
        let offset_minutes = match bytes.get(index)? {
            b'Z' | b'z' if index + 1 == bytes.len() => 0,
            sign @ (b'+' | b'-') if index + 6 == bytes.len() && bytes[index + 3] == b':' => {
                let hours = number(index + 1..index + 3)?;
                let minutes = number(index + 4..index + 6)?;
                if hours > 23 || minutes > 59 {
                    return None;
                }
                let total = hours * 60 + minutes;
                if *sign == b'-' { -total } else { total }
            }
            _ => return None,
        };
        let days = days_from_civil(year, month, day);
        let seconds =
            days * 86_400 + hour * 3600 + minute * 60 + second.min(59) - offset_minutes * 60;
        let ms = seconds * 1000 + millis;
        Some(Self(Timestamp::from_unix_millis(
            u64::try_from(ms).unwrap_or(0),
        )))
    }
}

const fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

const fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 (Howard Hinnant's algorithm).
const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The date of a day since 1970-01-01.
const fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

impl fmt::Display for MuteUntil {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_rfc3339())
    }
}

impl Serialize for MuteUntil {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_rfc3339())
    }
}

impl<'de> Deserialize<'de> for MuteUntil {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse_rfc3339(&text).ok_or_else(|| {
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&text),
                &"an RFC 3339 date-time",
            )
        })
    }
}

/// What a notification is about (contract §1). Also the `type` of a push.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotificationKind {
    /// A channel message, by level.
    Message,
    /// A message mentioning me.
    Mention,
    /// A reply to one of my messages that pings me.
    Reply,
    /// A message in a DM or group DM.
    Dm,
    /// Someone asked to be friends.
    FriendRequest,
    /// The guild owner wakes me up ("Uyandır").
    Nudge,
}

/// The per-type switches of [`GlobalNotificationSettings::types`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationTypes {
    /// Channel messages (by level).
    #[serde(default = "yes")]
    pub message: bool,
    /// Mentions.
    #[serde(default = "yes")]
    pub mention: bool,
    /// Replies to me.
    #[serde(default = "yes")]
    pub reply: bool,
    /// DMs and group DMs.
    #[serde(default = "yes")]
    pub dm: bool,
    /// Friend requests.
    #[serde(default = "yes")]
    pub friend_request: bool,
    /// Uyandır.
    #[serde(default = "yes")]
    pub nudge: bool,
}

impl Default for NotificationTypes {
    fn default() -> Self {
        Self {
            message: true,
            mention: true,
            reply: true,
            dm: true,
            friend_request: true,
            nudge: true,
        }
    }
}

impl NotificationTypes {
    /// The switch of `kind`.
    #[must_use]
    pub const fn allows(&self, kind: NotificationKind) -> bool {
        match kind {
            NotificationKind::Message => self.message,
            NotificationKind::Mention => self.mention,
            NotificationKind::Reply => self.reply,
            NotificationKind::Dm => self.dm,
            NotificationKind::FriendRequest => self.friend_request,
            NotificationKind::Nudge => self.nudge,
        }
    }
}

const fn yes() -> bool {
    true
}

/// Switches for every device. Defaults: everything on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalNotificationSettings {
    /// Native notifications on computers.
    #[serde(default = "yes")]
    pub desktop: bool,
    /// Pushes and local notifications on phones.
    #[serde(default = "yes")]
    pub mobile_push: bool,
    /// Per type.
    #[serde(default)]
    pub types: NotificationTypes,
    /// The author and the start of the text; off: only "Yeni mesaj".
    #[serde(default = "yes")]
    pub previews: bool,
    /// Notification sounds.
    #[serde(default = "yes")]
    pub sound: bool,
    /// Nothing notifies while the status is Rahatsız etmeyin (Uyandır keeps
    /// its own switch).
    #[serde(default = "yes")]
    pub quiet_when_dnd: bool,
}

impl Default for GlobalNotificationSettings {
    fn default() -> Self {
        Self {
            desktop: true,
            mobile_push: true,
            types: NotificationTypes::default(),
            previews: true,
            sound: true,
            quiet_when_dnd: true,
        }
    }
}

/// The level and mute of a category or channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeNotificationSettings {
    /// What it notifies of.
    #[serde(default)]
    pub level: NotificationLevel,
    /// Silent until then; `None`: not muted.
    #[serde(default)]
    pub mute_until: Option<MuteUntil>,
}

impl ScopeNotificationSettings {
    /// Whether it holds nothing (inherits and is not muted).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.level == NotificationLevel::Inherit && self.mute_until.is_none()
    }
}

/// A guild's level and mute, and its categories' and channels' overrides.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildNotificationSettings {
    /// The guild's level; [`NotificationLevel::Inherit`]: by its size
    /// ([`default_guild_level`]).
    #[serde(default)]
    pub level: NotificationLevel,
    /// The whole guild is silent until then.
    #[serde(default)]
    pub mute_until: Option<MuteUntil>,
    /// Category overrides by category (channel) ID.
    #[serde(default)]
    pub categories: BTreeMap<Snowflake, ScopeNotificationSettings>,
    /// Channel overrides by channel ID.
    #[serde(default)]
    pub channels: BTreeMap<Snowflake, ScopeNotificationSettings>,
}

impl GuildNotificationSettings {
    /// Whether it holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.level == NotificationLevel::Inherit
            && self.mute_until.is_none()
            && self.categories.is_empty()
            && self.channels.is_empty()
    }
}

/// A DM's mute (DMs have no levels).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DmNotificationSettings {
    /// Silent until then.
    #[serde(default)]
    pub mute_until: Option<MuteUntil>,
}

/// A user's notification settings (contract §2). Only overrides are
/// stored: a guild, category, channel or DM without an entry inherits.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationSettings {
    /// For every device.
    #[serde(default)]
    pub global: GlobalNotificationSettings,
    /// Per guild.
    #[serde(default)]
    pub guilds: BTreeMap<Snowflake, GuildNotificationSettings>,
    /// Per DM channel.
    #[serde(default)]
    pub dms: BTreeMap<Snowflake, DmNotificationSettings>,
}

/// Where a candidate notification comes from, for levels and mutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotificationScope {
    /// A guild channel.
    Guild {
        /// The guild.
        guild_id: Snowflake,
        /// The channel's category, if any.
        category_id: Option<Snowflake>,
        /// The channel.
        channel_id: Snowflake,
        /// The guild's member count (for the default level).
        member_count: usize,
    },
    /// A DM or group DM.
    Dm {
        /// The DM channel.
        channel_id: Snowflake,
    },
    /// Neither (friend requests, nudges): no levels or mutes apply.
    Account,
}

/// Which device kind decides (the global `desktop` or `mobilePush` switch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotificationSurface {
    /// A computer.
    Desktop,
    /// A phone or tablet (pushes and local notifications).
    Mobile,
}

impl NotificationSettings {
    /// The effective level of a guild channel: the narrowest of channel,
    /// category and guild that is not `inherit`, else the size default.
    /// Never [`NotificationLevel::Inherit`].
    #[must_use]
    pub fn level_of(
        &self,
        guild_id: Snowflake,
        category_id: Option<Snowflake>,
        channel_id: Snowflake,
        member_count: usize,
    ) -> NotificationLevel {
        let Some(guild) = self.guilds.get(&guild_id) else {
            return default_guild_level(member_count);
        };
        let channel = guild.channels.get(&channel_id).map(|scope| scope.level);
        let category = category_id
            .and_then(|id| guild.categories.get(&id))
            .map(|scope| scope.level);
        [channel, category, Some(guild.level)]
            .into_iter()
            .flatten()
            .find(|level| *level != NotificationLevel::Inherit)
            .unwrap_or_else(|| default_guild_level(member_count))
    }

    /// Whether the guild, the category or the channel is muted at `now`.
    #[must_use]
    pub fn is_guild_muted(
        &self,
        guild_id: Snowflake,
        category_id: Option<Snowflake>,
        channel_id: Snowflake,
        now: Timestamp,
    ) -> bool {
        let Some(guild) = self.guilds.get(&guild_id) else {
            return false;
        };
        let active = |mute: Option<MuteUntil>| mute.is_some_and(|until| until.is_active(now));
        active(guild.mute_until)
            || category_id
                .and_then(|id| guild.categories.get(&id))
                .is_some_and(|scope| active(scope.mute_until))
            || guild
                .channels
                .get(&channel_id)
                .is_some_and(|scope| active(scope.mute_until))
    }

    /// Whether a DM is muted at `now`.
    #[must_use]
    pub fn is_dm_muted(&self, channel_id: Snowflake, now: Timestamp) -> bool {
        self.dms
            .get(&channel_id)
            .and_then(|dm| dm.mute_until)
            .is_some_and(|until| until.is_active(now))
    }

    /// The rules of contract §1 that depend on the settings alone: the
    /// device switch, Rahatsız etmeyin, mutes, levels and the type
    /// switches. `kinds` are what the event is, most specific first (a
    /// pinging reply: `[Reply, Mention, Message]`); the first one allowed
    /// is what it notifies as. `None`: it does not notify.
    ///
    /// Uyandır ignores levels, mutes and Rahatsız etmeyin: only its own
    /// switch (and the device switch) count.
    #[must_use]
    pub fn decide(
        &self,
        surface: NotificationSurface,
        kinds: &[NotificationKind],
        scope: NotificationScope,
        do_not_disturb: bool,
        now: Timestamp,
    ) -> Option<NotificationKind> {
        let global = &self.global;
        let device = match surface {
            NotificationSurface::Desktop => global.desktop,
            NotificationSurface::Mobile => global.mobile_push,
        };
        if !device {
            return None;
        }
        let level = match scope {
            NotificationScope::Guild {
                guild_id,
                category_id,
                channel_id,
                member_count,
            } => {
                if self.is_guild_muted(guild_id, category_id, channel_id, now) {
                    return None;
                }
                self.level_of(guild_id, category_id, channel_id, member_count)
            }
            NotificationScope::Dm { channel_id } => {
                if self.is_dm_muted(channel_id, now) {
                    return None;
                }
                NotificationLevel::All
            }
            NotificationScope::Account => NotificationLevel::All,
        };
        kinds.iter().copied().find(|kind| {
            let quiet = do_not_disturb && global.quiet_when_dnd && *kind != NotificationKind::Nudge;
            let by_level = match level {
                NotificationLevel::All | NotificationLevel::Inherit => true,
                NotificationLevel::Mentions => {
                    matches!(kind, NotificationKind::Mention | NotificationKind::Reply)
                }
                NotificationLevel::None => false,
            };
            !quiet && by_level && global.types.allows(*kind)
        })
    }

    /// Applies a PATCH: absent fields stay, `null` deletes, empty entries
    /// are dropped.
    pub fn apply(&mut self, patch: NotificationSettingsPatch) {
        if let Some(global) = patch.global {
            global.apply(&mut self.global);
        }
        for (guild_id, change) in patch.guilds.unwrap_or_default() {
            match change {
                None => {
                    self.guilds.remove(&guild_id);
                }
                Some(change) => {
                    let entry = self.guilds.entry(guild_id).or_default();
                    change.apply(entry);
                    if entry.is_empty() {
                        self.guilds.remove(&guild_id);
                    }
                }
            }
        }
        for (channel_id, change) in patch.dms.unwrap_or_default() {
            match change.and_then(|change| change.mute_until) {
                None | Some(None) => {
                    self.dms.remove(&channel_id);
                }
                Some(Some(until)) => {
                    self.dms.insert(
                        channel_id,
                        DmNotificationSettings {
                            mute_until: Some(until),
                        },
                    );
                }
            }
        }
    }

    /// Drops mutes that ended before `now` and the entries left empty.
    pub fn prune(&mut self, now: Timestamp) {
        let ended = |mute: &mut Option<MuteUntil>| {
            if mute.is_some_and(|until| !until.is_active(now)) {
                *mute = None;
            }
        };
        for guild in self.guilds.values_mut() {
            ended(&mut guild.mute_until);
            for scope in guild
                .categories
                .values_mut()
                .chain(guild.channels.values_mut())
            {
                ended(&mut scope.mute_until);
            }
            guild.categories.retain(|_, scope| !scope.is_empty());
            guild.channels.retain(|_, scope| !scope.is_empty());
        }
        self.guilds.retain(|_, guild| !guild.is_empty());
        for dm in self.dms.values_mut() {
            ended(&mut dm.mute_until);
        }
        self.dms.retain(|_, dm| dm.mute_until.is_some());
    }
}

/// `PATCH /v1/users/@me/notification-settings`: the settings' shape with
/// every field optional. `null` for a guild, category, channel or DM
/// deletes its override; `null` for a `level` makes it `inherit`, for a
/// `muteUntil` unmutes.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationSettingsPatch {
    /// Global switches to change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global: Option<GlobalNotificationPatch>,
    /// Guild entries to change (`null`: delete).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guilds: Option<BTreeMap<Snowflake, Option<GuildNotificationPatch>>>,
    /// DM entries to change (`null`: delete).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dms: Option<BTreeMap<Snowflake, Option<DmNotificationPatch>>>,
}

/// Global switches of a PATCH.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalNotificationPatch {
    /// [`GlobalNotificationSettings::desktop`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desktop: Option<bool>,
    /// [`GlobalNotificationSettings::mobile_push`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mobile_push: Option<bool>,
    /// Type switches to change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub types: Option<NotificationTypesPatch>,
    /// [`GlobalNotificationSettings::previews`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previews: Option<bool>,
    /// [`GlobalNotificationSettings::sound`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<bool>,
    /// [`GlobalNotificationSettings::quiet_when_dnd`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quiet_when_dnd: Option<bool>,
}

impl GlobalNotificationPatch {
    fn apply(self, global: &mut GlobalNotificationSettings) {
        let set = |target: &mut bool, value: Option<bool>| {
            if let Some(value) = value {
                *target = value;
            }
        };
        set(&mut global.desktop, self.desktop);
        set(&mut global.mobile_push, self.mobile_push);
        set(&mut global.previews, self.previews);
        set(&mut global.sound, self.sound);
        set(&mut global.quiet_when_dnd, self.quiet_when_dnd);
        if let Some(types) = self.types {
            let t = &mut global.types;
            set(&mut t.message, types.message);
            set(&mut t.mention, types.mention);
            set(&mut t.reply, types.reply);
            set(&mut t.dm, types.dm);
            set(&mut t.friend_request, types.friend_request);
            set(&mut t.nudge, types.nudge);
        }
    }
}

/// Type switches of a PATCH.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(missing_docs)]
pub struct NotificationTypesPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mention: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dm: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub friend_request: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nudge: Option<bool>,
}

/// A guild entry of a PATCH.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildNotificationPatch {
    /// The level (`null`: inherit).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    pub level: Option<Option<NotificationLevel>>,
    /// The mute (`null`: unmute).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    pub mute_until: Option<Option<MuteUntil>>,
    /// Category entries to change (`null`: delete).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub categories: Option<BTreeMap<Snowflake, Option<ScopeNotificationPatch>>>,
    /// Channel entries to change (`null`: delete).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<BTreeMap<Snowflake, Option<ScopeNotificationPatch>>>,
}

impl GuildNotificationPatch {
    fn apply(self, guild: &mut GuildNotificationSettings) {
        if let Some(level) = self.level {
            guild.level = level.unwrap_or_default();
        }
        if let Some(mute) = self.mute_until {
            guild.mute_until = mute;
        }
        for (map, changes) in [
            (&mut guild.categories, self.categories),
            (&mut guild.channels, self.channels),
        ] {
            for (id, change) in changes.unwrap_or_default() {
                match change {
                    None => {
                        map.remove(&id);
                    }
                    Some(change) => {
                        let entry = map.entry(id).or_default();
                        change.apply(entry);
                        if entry.is_empty() {
                            map.remove(&id);
                        }
                    }
                }
            }
        }
    }

    /// The category and channel IDs it names.
    #[must_use]
    pub fn scope_ids(&self) -> Vec<Snowflake> {
        self.categories
            .iter()
            .chain(self.channels.iter())
            .flat_map(BTreeMap::keys)
            .copied()
            .collect()
    }
}

/// A category or channel entry of a PATCH.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeNotificationPatch {
    /// The level (`null`: inherit).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    pub level: Option<Option<NotificationLevel>>,
    /// The mute (`null`: unmute).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    pub mute_until: Option<Option<MuteUntil>>,
}

impl ScopeNotificationPatch {
    fn apply(self, scope: &mut ScopeNotificationSettings) {
        if let Some(level) = self.level {
            scope.level = level.unwrap_or_default();
        }
        if let Some(mute) = self.mute_until {
            scope.mute_until = mute;
        }
    }
}

/// A DM entry of a PATCH.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DmNotificationPatch {
    /// The mute (`null`: unmute).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    pub mute_until: Option<Option<MuteUntil>>,
}

/// The texts every platform shows (contract §3), in Turkish like the apps.
/// The server writes pushes in the recipient's language with the `*_in`
/// functions (ADR-083); the Turkish constants are what they give for
/// [`Locale::Tr`], which the apps use for their local notifications.
pub mod text {
    use crate::locale::Locale;
    use crate::snowflake::Snowflake;

    /// [`NEW_MESSAGE`] in `locale`.
    #[must_use]
    pub const fn new_message_in(locale: Locale) -> &'static str {
        match locale {
            Locale::Tr => NEW_MESSAGE,
            Locale::En => "New message",
            Locale::De => "Neue Nachricht",
            Locale::Fr => "Nouveau message",
            Locale::Es => "Nuevo mensaje",
        }
    }

    /// [`ATTACHMENT`] in `locale`.
    #[must_use]
    pub const fn attachment_in(locale: Locale) -> &'static str {
        match locale {
            Locale::Tr => ATTACHMENT,
            Locale::En => "📎 File",
            Locale::De => "📎 Datei",
            Locale::Fr => "📎 Fichier",
            Locale::Es => "📎 Archivo",
        }
    }

    /// [`FRIEND_REQUEST_TITLE`] in `locale`.
    #[must_use]
    pub const fn friend_request_title_in(locale: Locale) -> &'static str {
        match locale {
            Locale::Tr => FRIEND_REQUEST_TITLE,
            Locale::En => "Friend request",
            Locale::De => "Freundschaftsanfrage",
            Locale::Fr => "Demande d’ami",
            Locale::Es => "Solicitud de amistad",
        }
    }

    /// [`body`] in `locale`: `"{author}: {preview}"`, or
    /// [`new_message_in`] without a preview.
    #[must_use]
    pub fn body_in(locale: Locale, author: &str, preview: Option<&str>) -> String {
        match preview {
            Some(text) => format!("{}: {text}", clean(author)),
            None => new_message_in(locale).to_owned(),
        }
    }

    /// [`friend_request_body`] in `locale`.
    #[must_use]
    pub fn friend_request_body_in(locale: Locale, name: &str) -> String {
        let name = clean(name);
        match locale {
            Locale::Tr => format!("{name} sana arkadaşlık isteği gönderdi"),
            Locale::En => format!("{name} sent you a friend request"),
            Locale::De => format!("{name} hat dir eine Freundschaftsanfrage geschickt"),
            Locale::Fr => format!("{name} t’a envoyé une demande d’ami"),
            Locale::Es => format!("{name} te ha enviado una solicitud de amistad"),
        }
    }

    /// [`nudge_body`] in `locale`.
    #[must_use]
    pub fn nudge_body_in(locale: Locale, name: &str) -> String {
        let name = clean(name);
        match locale {
            Locale::Tr => format!("{name} seni uyandırıyor!"),
            Locale::En => format!("{name} is nudging you!"),
            Locale::De => format!("{name} stupst dich an!"),
            // French puts a narrow no-break space before "!".
            Locale::Fr => format!("{name} te fait signe\u{202f}!"),
            Locale::Es => format!("¡{name} te está dando un toque!"),
        }
    }

    /// The body with previews off.
    pub const NEW_MESSAGE: &str = "Yeni mesaj";
    /// The toast when a notification's message was deleted.
    pub const MESSAGE_DELETED: &str = "Mesaj silinmiş";
    /// A message with files only.
    pub const ATTACHMENT: &str = "📎 Dosya";
    /// The title of a friend request.
    pub const FRIEND_REQUEST_TITLE: &str = "Arkadaşlık isteği";
    /// The title and body of "Test bildirimi gönder".
    pub const TEST_TITLE: &str = "Evre";
    /// The body of the test notification.
    pub const TEST_BODY: &str = "Bildirimler çalışıyor. Tıklayınca Evre açılır.";

    /// [`TEST_BODY`] in `locale`.
    #[must_use]
    pub const fn test_body_in(locale: Locale) -> &'static str {
        match locale {
            Locale::Tr => TEST_BODY,
            Locale::En => "Notifications work. Click to open Evre.",
            Locale::De => "Benachrichtigungen funktionieren. Klicke, um Evre zu öffnen.",
            Locale::Fr => "Les notifications fonctionnent. Clique pour ouvrir Evre.",
            Locale::Es => "Las notificaciones funcionan. Haz clic para abrir Evre.",
        }
    }
    /// Longest preview text, in characters.
    pub const PREVIEW_CHARS: usize = 180;

    /// `"{Topluluk} · #{kanal}"`.
    #[must_use]
    pub fn channel_title(guild: &str, channel: &str) -> String {
        format!("{} · #{}", clean(guild), clean(channel))
    }

    /// `"{yazar}: {mesaj}"`, or [`NEW_MESSAGE`] without a preview.
    #[must_use]
    pub fn body(author: &str, preview: Option<&str>) -> String {
        match preview {
            Some(text) => format!("{}: {text}", clean(author)),
            None => NEW_MESSAGE.to_owned(),
        }
    }

    /// A friend request's body.
    #[must_use]
    pub fn friend_request_body(name: &str) -> String {
        friend_request_body_in(Locale::Tr, name)
    }

    /// Uyandır's body.
    #[must_use]
    pub fn nudge_body(name: &str) -> String {
        nudge_body_in(Locale::Tr, name)
    }

    /// Text without bidi overrides and control characters (notifications
    /// are outside the chat's isolation), on one line.
    #[must_use]
    pub fn clean(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut space = false;
        for c in text.chars() {
            if matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}')
            {
                continue;
            }
            if c.is_whitespace() || c.is_control() {
                space = !out.is_empty();
                continue;
            }
            if space {
                out.push(' ');
                space = false;
            }
            out.push(c);
        }
        out
    }

    /// The preview of a message: `<@id>` as `@name` (`name_of`; unknown
    /// IDs stay as they are), cleaned, at most [`PREVIEW_CHARS`]
    /// characters ending in `…` when cut; [`ATTACHMENT`] for files only.
    #[must_use]
    pub fn preview(
        content: &str,
        attachments: usize,
        name_of: impl Fn(Snowflake) -> Option<String>,
    ) -> String {
        preview_in(Locale::Tr, content, attachments, name_of)
    }

    /// [`preview`] in `locale` (only the files-only text changes).
    #[must_use]
    pub fn preview_in(
        locale: Locale,
        content: &str,
        attachments: usize,
        name_of: impl Fn(Snowflake) -> Option<String>,
    ) -> String {
        let mut named = String::with_capacity(content.len());
        let mut rest = content;
        while let Some(start) = rest.find("<@") {
            named.push_str(&rest[..start]);
            let after = &rest[start + 2..];
            let end = after.find('>');
            let id = end.and_then(|end| after[..end].parse::<u64>().ok());
            match (end, id) {
                (Some(end), Some(id)) => {
                    match name_of(Snowflake::new(id)) {
                        Some(name) => {
                            named.push('@');
                            named.push_str(&name);
                        }
                        None => named.push_str(&rest[start..start + 2 + end + 1]),
                    }
                    rest = &after[end + 1..];
                }
                _ => {
                    named.push_str("<@");
                    rest = after;
                }
            }
        }
        named.push_str(rest);
        let text = clean(&named);
        if text.is_empty() {
            return if attachments > 0 {
                attachment_in(locale).to_owned()
            } else {
                String::new()
            };
        }
        if text.chars().count() <= PREVIEW_CHARS {
            return text;
        }
        let mut cut: String = text.chars().take(PREVIEW_CHARS - 1).collect();
        cut.truncate(cut.trim_end().len());
        cut.push('…');
        cut
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(raw: u64) -> Snowflake {
        Snowflake::new(raw)
    }

    fn at(text: &str) -> MuteUntil {
        MuteUntil::parse_rfc3339(text).unwrap()
    }

    #[test]
    fn rfc3339_round_trips_and_rejects_garbage() {
        assert_eq!(at("1970-01-01T00:00:00Z").timestamp().unix_millis(), 0);
        assert_eq!(
            at("2026-10-28T08:00:00Z").to_rfc3339(),
            "2026-10-28T08:00:00Z"
        );
        assert_eq!(at("2026-10-28T11:00:00+03:00"), at("2026-10-28T08:00:00Z"));
        assert_eq!(
            at("2026-10-28T08:00:00.5Z").to_rfc3339(),
            "2026-10-28T08:00:00.500Z"
        );
        assert_eq!(
            at("2024-02-29T23:59:59-01:30").to_rfc3339(),
            "2024-03-01T01:29:59Z"
        );
        assert_eq!(at("9999-12-31T23:59:59Z"), MuteUntil::FOREVER);
        assert_eq!(MuteUntil::FOREVER.to_rfc3339(), "9999-12-31T23:59:59Z");
        assert_eq!(at("1969-12-31T00:00:00Z").timestamp().unix_millis(), 0);
        for bad in [
            "",
            "2026-10-28",
            "2026-10-28T08:00:00",
            "2026-13-01T00:00:00Z",
            "2025-02-29T00:00:00Z",
            "2026-10-28T24:00:00Z",
            "2026-10-28T08:00:00+0300",
            "2026-10-28T08:00:00Zjunk",
            "２026-10-28T08:00:00Z",
        ] {
            assert_eq!(MuteUntil::parse_rfc3339(bad), None, "{bad}");
        }
        let json = serde_json::to_string(&Some(at("2026-10-28T08:00:00Z"))).unwrap();
        assert_eq!(json, r#""2026-10-28T08:00:00Z""#);
        assert!(serde_json::from_str::<MuteUntil>("1730102400000").is_err());
    }

    #[test]
    fn presets_end_where_the_ui_says() {
        // 2026-10-28T10:00:00Z.
        let now = at("2026-10-28T10:00:00Z").timestamp();
        let preset = |p, offset| MuteUntil::preset(p, now, offset).to_rfc3339();
        assert_eq!(preset(MutePreset::Minutes15, 0), "2026-10-28T10:15:00Z");
        assert_eq!(preset(MutePreset::Hour1, 0), "2026-10-28T11:00:00Z");
        assert_eq!(preset(MutePreset::Forever, 0), "9999-12-31T23:59:59Z");
        // 13:00 in Istanbul: tomorrow 08:00 local = 05:00Z.
        assert_eq!(
            preset(MutePreset::UntilTomorrow, 180),
            "2026-10-29T05:00:00Z"
        );
        // 03:00 local (UTC-7 at 10:00Z): today 08:00 local is still ahead.
        assert_eq!(
            preset(MutePreset::UntilTomorrow, -420),
            "2026-10-28T15:00:00Z"
        );
    }

    #[test]
    fn defaults_are_everything_on() {
        let settings: NotificationSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, NotificationSettings::default());
        let g = settings.global;
        assert!(g.desktop && g.mobile_push && g.previews && g.sound && g.quiet_when_dnd);
        assert_eq!(g.types, NotificationTypes::default());
        let json = serde_json::to_value(&settings).unwrap();
        assert_eq!(
            json["global"]["types"],
            serde_json::json!({"message": true, "mention": true, "reply": true, "dm": true, "friendRequest": true, "nudge": true})
        );
        assert_eq!(json["global"]["quietWhenDnd"], true);
        assert_eq!(json["guilds"], serde_json::json!({}));
        let level: NotificationLevel = serde_json::from_str(r#""someday""#).unwrap();
        assert_eq!(level, NotificationLevel::Inherit);
    }

    #[test]
    fn the_narrowest_level_wins_and_size_sets_the_default() {
        let mut settings = NotificationSettings::default();
        let (guild, category, channel) = (id(1), id(2), id(3));
        assert_eq!(
            settings.level_of(guild, Some(category), channel, 100),
            NotificationLevel::All
        );
        assert_eq!(
            settings.level_of(guild, Some(category), channel, 101),
            NotificationLevel::Mentions
        );
        let entry = settings.guilds.entry(guild).or_default();
        entry.level = NotificationLevel::None;
        assert_eq!(
            settings.level_of(guild, Some(category), channel, 5),
            NotificationLevel::None
        );
        let entry = settings.guilds.get_mut(&guild).unwrap();
        entry.categories.insert(
            category,
            ScopeNotificationSettings {
                level: NotificationLevel::Mentions,
                mute_until: None,
            },
        );
        assert_eq!(
            settings.level_of(guild, Some(category), channel, 5),
            NotificationLevel::Mentions
        );
        assert_eq!(
            settings.level_of(guild, None, channel, 5),
            NotificationLevel::None
        );
        settings.guilds.get_mut(&guild).unwrap().channels.insert(
            channel,
            ScopeNotificationSettings {
                level: NotificationLevel::All,
                mute_until: None,
            },
        );
        assert_eq!(
            settings.level_of(guild, Some(category), channel, 500),
            NotificationLevel::All
        );
        // Inherit at every scope: the size default.
        let mut inherit = NotificationSettings::default();
        inherit.guilds.entry(guild).or_default().channels.insert(
            channel,
            ScopeNotificationSettings {
                level: NotificationLevel::Inherit,
                mute_until: Some(MuteUntil::FOREVER),
            },
        );
        assert_eq!(
            inherit.level_of(guild, None, channel, 500),
            NotificationLevel::Mentions
        );
    }

    fn guild_scope(members: usize) -> NotificationScope {
        NotificationScope::Guild {
            guild_id: id(1),
            category_id: Some(id(2)),
            channel_id: id(3),
            member_count: members,
        }
    }

    const MESSAGE: &[NotificationKind] = &[NotificationKind::Message];
    const MENTION: &[NotificationKind] = &[NotificationKind::Mention, NotificationKind::Message];
    const REPLY: &[NotificationKind] = &[
        NotificationKind::Reply,
        NotificationKind::Mention,
        NotificationKind::Message,
    ];

    #[test]
    fn decide_applies_levels_mutes_switches_and_dnd() {
        use NotificationKind as K;
        use NotificationSurface::{Desktop, Mobile};
        let now = at("2026-10-28T10:00:00Z").timestamp();
        let mut s = NotificationSettings::default();
        let decide =
            |s: &NotificationSettings, kinds, scope, dnd| s.decide(Desktop, kinds, scope, dnd, now);
        assert_eq!(
            decide(&s, MESSAGE, guild_scope(10), false),
            Some(K::Message)
        );
        // Large guilds: mentions and replies only.
        assert_eq!(decide(&s, MESSAGE, guild_scope(200), false), None);
        assert_eq!(
            decide(&s, MENTION, guild_scope(200), false),
            Some(K::Mention)
        );
        assert_eq!(decide(&s, REPLY, guild_scope(200), false), Some(K::Reply));
        // `none` silences mentions too.
        s.guilds.entry(id(1)).or_default().level = NotificationLevel::None;
        assert_eq!(decide(&s, REPLY, guild_scope(10), false), None);
        s.guilds.clear();
        // Mutes: guild, category and channel; an ended mute does nothing.
        for target in 0..3 {
            let mut muted = NotificationSettings::default();
            let guild = muted.guilds.entry(id(1)).or_default();
            let until = Some(at("2026-10-28T10:15:00Z"));
            match target {
                0 => guild.mute_until = until,
                1 => {
                    guild.categories.insert(
                        id(2),
                        ScopeNotificationSettings {
                            level: NotificationLevel::Inherit,
                            mute_until: until,
                        },
                    );
                }
                _ => {
                    guild.channels.insert(
                        id(3),
                        ScopeNotificationSettings {
                            level: NotificationLevel::All,
                            mute_until: until,
                        },
                    );
                }
            }
            assert_eq!(
                decide(&muted, REPLY, guild_scope(10), false),
                None,
                "scope {target}"
            );
            let later = at("2026-10-28T10:16:00Z").timestamp();
            assert_eq!(
                muted.decide(Desktop, REPLY, guild_scope(10), false, later),
                Some(K::Reply)
            );
        }
        // A guild mute never silences DMs, which have their own.
        let mut muted = NotificationSettings::default();
        muted.guilds.entry(id(1)).or_default().mute_until = Some(MuteUntil::FOREVER);
        let dm = NotificationScope::Dm { channel_id: id(9) };
        assert_eq!(decide(&muted, &[K::Dm], dm, false), Some(K::Dm));
        muted.dms.insert(
            id(9),
            DmNotificationSettings {
                mute_until: Some(MuteUntil::FOREVER),
            },
        );
        assert_eq!(decide(&muted, &[K::Dm], dm, false), None);
        // Type switches: a reply falls back to the message class at `all`.
        s.global.types.reply = false;
        s.global.types.mention = false;
        assert_eq!(decide(&s, REPLY, guild_scope(10), false), Some(K::Message));
        assert_eq!(decide(&s, REPLY, guild_scope(200), false), None);
        s.global.types = NotificationTypes::default();
        s.global.types.friend_request = false;
        assert_eq!(
            decide(&s, &[K::FriendRequest], NotificationScope::Account, false),
            None
        );
        s.global.types = NotificationTypes::default();
        // Rahatsız etmeyin: quiet unless turned off; Uyandır keeps its switch.
        assert_eq!(decide(&s, &[K::Dm], dm, true), None);
        assert_eq!(
            decide(&s, &[K::Nudge], NotificationScope::Account, true),
            Some(K::Nudge)
        );
        s.global.types.nudge = false;
        assert_eq!(
            decide(&s, &[K::Nudge], NotificationScope::Account, false),
            None
        );
        s.global.quiet_when_dnd = false;
        assert_eq!(decide(&s, &[K::Dm], dm, true), Some(K::Dm));
        // The device switches.
        s.global.desktop = false;
        assert_eq!(decide(&s, &[K::Dm], dm, false), None);
        assert_eq!(s.decide(Mobile, &[K::Dm], dm, false, now), Some(K::Dm));
        s.global.mobile_push = false;
        assert_eq!(s.decide(Mobile, &[K::Dm], dm, false, now), None);
    }

    #[test]
    fn patches_merge_and_null_deletes() {
        let mut settings = NotificationSettings::default();
        let patch: NotificationSettingsPatch = serde_json::from_value(serde_json::json!({
            "global": {"previews": false, "types": {"message": false}},
            "guilds": {
                "1": {"level": "mentions", "muteUntil": "2026-10-28T10:15:00Z",
                      "categories": {"2": {"level": "none"}},
                      "channels": {"3": {"muteUntil": "9999-12-31T23:59:59Z"}}},
                "4": {"level": "all"}
            },
            "dms": {"9": {"muteUntil": "9999-12-31T23:59:59Z"}}
        }))
        .unwrap();
        settings.apply(patch);
        assert!(!settings.global.previews && !settings.global.types.message);
        assert!(settings.global.types.mention && settings.global.sound);
        let guild = &settings.guilds[&id(1)];
        assert_eq!(guild.level, NotificationLevel::Mentions);
        assert_eq!(guild.categories[&id(2)].level, NotificationLevel::None);
        assert_eq!(guild.channels[&id(3)].mute_until, Some(MuteUntil::FOREVER));
        assert_eq!(settings.guilds[&id(4)].level, NotificationLevel::All);
        assert!(settings.dms.contains_key(&id(9)));

        let patch: NotificationSettingsPatch = serde_json::from_value(serde_json::json!({
            "guilds": {
                "1": {"muteUntil": null, "categories": {"2": null}, "channels": {"3": {"muteUntil": null}}},
                "4": null
            },
            "dms": {"9": {"muteUntil": null}}
        }))
        .unwrap();
        settings.apply(patch);
        let guild = &settings.guilds[&id(1)];
        assert_eq!(
            guild.level,
            NotificationLevel::Mentions,
            "absent fields stay"
        );
        assert_eq!(guild.mute_until, None);
        assert!(guild.categories.is_empty() && guild.channels.is_empty());
        assert!(!settings.guilds.contains_key(&id(4)));
        assert!(settings.dms.is_empty());
        // `level: null` back to inherit drops the empty entry.
        let patch: NotificationSettingsPatch =
            serde_json::from_value(serde_json::json!({"guilds": {"1": {"level": null}}})).unwrap();
        settings.apply(patch);
        assert!(settings.guilds.is_empty());
        // The round trip keeps tri-state fields.
        let patch = NotificationSettingsPatch {
            guilds: Some(BTreeMap::from([(
                id(5),
                Some(GuildNotificationPatch {
                    mute_until: Some(None),
                    ..GuildNotificationPatch::default()
                }),
            )])),
            ..NotificationSettingsPatch::default()
        };
        let json = serde_json::to_value(&patch).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"guilds": {"5": {"muteUntil": null}}})
        );
        assert_eq!(
            serde_json::from_value::<NotificationSettingsPatch>(json).unwrap(),
            patch
        );
    }

    #[test]
    fn prune_drops_ended_mutes() {
        let mut settings = NotificationSettings::default();
        let guild = settings.guilds.entry(id(1)).or_default();
        guild.mute_until = Some(at("2026-10-28T09:00:00Z"));
        guild.channels.insert(
            id(3),
            ScopeNotificationSettings {
                level: NotificationLevel::All,
                mute_until: Some(at("2026-10-28T09:00:00Z")),
            },
        );
        settings.dms.insert(
            id(9),
            DmNotificationSettings {
                mute_until: Some(at("2026-10-28T09:00:00Z")),
            },
        );
        settings.prune(at("2026-10-28T10:00:00Z").timestamp());
        let guild = &settings.guilds[&id(1)];
        assert_eq!(guild.mute_until, None);
        assert_eq!(guild.channels[&id(3)].level, NotificationLevel::All);
        assert!(settings.dms.is_empty());
    }

    #[test]
    fn texts_exist_in_every_locale_and_turkish_matches_the_constants() {
        use crate::locale::Locale;
        assert_eq!(text::new_message_in(Locale::Tr), text::NEW_MESSAGE);
        assert_eq!(text::attachment_in(Locale::Tr), text::ATTACHMENT);
        assert_eq!(
            text::friend_request_title_in(Locale::Tr),
            text::FRIEND_REQUEST_TITLE
        );
        assert_eq!(
            text::friend_request_body("Ada"),
            "Ada sana arkadaşlık isteği gönderdi"
        );
        assert_eq!(text::nudge_body("Ada"), "Ada seni uyandırıyor!");
        assert_eq!(text::body_in(Locale::De, "Ada", None), "Neue Nachricht");
        assert_eq!(
            text::body_in(Locale::De, "Ada", Some("hallo")),
            "Ada: hallo"
        );
        assert_eq!(
            text::nudge_body_in(Locale::Es, "Ada\u{202E}"),
            "¡Ada te está dando un toque!"
        );
        let mut seen = std::collections::HashSet::new();
        for locale in Locale::ALL {
            assert!(
                seen.insert(text::new_message_in(locale)),
                "{locale}: its own text"
            );
            assert!(text::friend_request_body_in(locale, "Ada").contains("Ada"));
            assert!(text::nudge_body_in(locale, "Ada").contains("Ada"));
            assert!(text::attachment_in(locale).starts_with('📎'));
        }
    }

    #[test]
    fn texts_are_clean_and_short() {
        assert_eq!(
            text::channel_title("Çaka Çuka\u{202E}", "genel"),
            "Çaka Çuka · #genel"
        );
        assert_eq!(text::body("Deniz", Some("selam")), "Deniz: selam");
        assert_eq!(text::body("Deniz", None), "Yeni mesaj");
        let names = |id: Snowflake| (id == Snowflake::new(7)).then(|| "kaan".to_owned());
        assert_eq!(
            text::preview("selam <@7>  ve\n<@8> <@x", 0, names),
            "selam @kaan ve <@8> <@x"
        );
        assert_eq!(text::preview("", 2, names), "📎 Dosya");
        assert_eq!(text::preview("  ", 0, names), "");
        let long = "a".repeat(300);
        let cut = text::preview(&long, 0, names);
        assert_eq!(cut.chars().count(), text::PREVIEW_CHARS);
        assert!(cut.ends_with('…'));
    }
}
