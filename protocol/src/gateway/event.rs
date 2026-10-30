//! Dispatch events.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use crate::model::{
    Channel, Device, DmChannel, Guild, Interaction, Member, Message, Platform, PresenceStatus,
    Relationship, Role, StatusSetting, User, VoiceState,
};
use crate::rest::{GuildSnapshot, JoinRequestStatus, NotificationSettings, ReadState};
use crate::snowflake::Snowflake;

/// An event delivered in [`GatewayMessage::Dispatch`](super::GatewayMessage::Dispatch),
/// encoded as `{"t": "<type>", "d": {..}}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", content = "d", rename_all = "camelCase")]
pub enum Event {
    /// A message was sent. Carries the author's `nonce` so the outbox can
    /// match its pending send (§8.1).
    MessageCreate(Message),
    /// A message was edited.
    MessageUpdate(Message),
    /// A message was deleted.
    MessageDelete(MessageDelete),
    /// A channel was created.
    ChannelCreate(Channel),
    /// A channel changed (name, position, overwrites, voice settings, ...).
    ChannelUpdate(Channel),
    /// A channel was deleted.
    ChannelDelete(ChannelDelete),
    /// A user joined a guild.
    GuildMemberAdd(GuildMemberAdd),
    /// A member left, was kicked or was banned.
    GuildMemberRemove(GuildMemberRemove),
    /// A member's nickname or roles changed.
    GuildMemberUpdate(Member),
    /// A user joined, left or moved between voice channels, or changed mute,
    /// deafen or streaming state. Joining is always an explicit client action
    /// (§6.6); see [`VoiceState`].
    VoiceStateUpdate(VoiceState),
    /// A user's online status changed. Sent to every session that shares a
    /// guild with the user, and to the user's own sessions.
    PresenceUpdate(PresenceUpdate),
    /// The user changed their chosen status on one of their devices. Sent
    /// only to that user's sessions.
    StatusSettingUpdate(StatusSetting),
    /// A user's public profile changed (display name, avatar, banner, bio).
    /// Sent to every session that shares a guild with the user, and to the
    /// user's own sessions. Replaces the cached [`User`] everywhere.
    UserUpdate(User),
    /// A user started typing.
    TypingStart(TypingStart),
    /// The full state of one guild as seen by this session: sent after
    /// `ready` for every guild, after joining a guild, and when role or
    /// overwrite changes altered what the user can see. Replaces all cached
    /// state of that guild.
    GuildCreate(GuildSnapshot),
    /// Guild settings (name, plan, owner, icon, banner) changed.
    GuildUpdate(Guild),
    /// A role was created.
    GuildRoleCreate(Role),
    /// A role changed.
    GuildRoleUpdate(Role),
    /// A role was deleted.
    GuildRoleDelete(GuildRoleDelete),
    /// A DM appeared for this user: they opened it, were added to a group
    /// DM, or the first message of a 1:1 DM someone else opened arrived
    /// (sent just before that `messageCreate`). Sent only to the
    /// participants it appeared for.
    DmCreate(DmChannel),
    /// A DM's participants, owner or name changed (someone left a group).
    /// Sent to its participants.
    DmUpdate(DmChannel),
    /// This user left a group DM. Sent only to the leaver's sessions.
    DmDelete(DmDelete),
    /// A friend request was sent or received, or became a friendship
    /// (§8.5), or this user blocked `user`. Sent to the user it concerns
    /// (a block only to the blocker); replaces any earlier entry for the
    /// same `user`.
    RelationshipAdd(Relationship),
    /// A friend was removed, a request was cancelled or declined, or this
    /// user unblocked someone. Sent to both users, except for an unblock
    /// (only the blocker). Blocking a friend or someone with a pending
    /// request sends this to both first, exactly like unfriending.
    RelationshipRemove(RelationshipRemove),
    /// The guild owner wants to wake this user up ("Uyandır"): play a loud
    /// alarm. Sent only to the target user's sessions.
    Nudge(Nudge),
    /// The user marked a channel read on another device (§8.1): replace the
    /// channel's read state. Sent to the user's sessions of other login
    /// sessions (devices), not to the one that marked it.
    ReadStateUpdate(ReadState),
    /// Someone clicked a button under one of this bot's messages (§8.1
    /// interactions). Sent only to the sessions of the bot that owns the
    /// message; answer with `POST /v1/interactions/{id}/callback` in time.
    InteractionCreate(Interaction),
    /// A bot answered this user's interaction with a message only they see
    /// ([`Message::ephemeral`]). Sent only to that user's sessions; never
    /// stored, so it is not in history and counts as nothing unread.
    EphemeralMessage(Message),
    /// The user's notification settings changed on one of their devices
    /// (`docs/contracts/notifications.md` §2, there called
    /// `NOTIFICATION_SETTINGS_UPDATE`): the full new settings. Sent to every
    /// session of the user, the changing one included.
    NotificationSettingsUpdate(NotificationSettings),
    /// The guild is gone for this session: its owner deleted it (ADR-090).
    /// Drop everything cached of it. Sent to every session in the guild;
    /// each member also gets a `guildMemberRemove` naming themselves, which
    /// clients older than this event already handle as leaving.
    GuildDelete(GuildDelete),
    /// The guild's number of pending join requests changed (ADR-095): a
    /// request came, was withdrawn or decided. Sent to the sessions of the
    /// guild's members with `KICK_MEMBERS` (people only); replaces
    /// [`GuildSnapshot::pending_join_requests`].
    JoinRequestCreate(JoinRequestCount),
    /// Our join request was decided or withdrawn (ADR-095). Sent to the
    /// requester's sessions; `approved` also brings the guild's
    /// `guildCreate` (sent before this event).
    JoinRequestUpdate(JoinRequestUpdate),
    /// An event type introduced after this build. Produced only when decoding
    /// a dispatch, so older clients can skip new events; never sent (encoding
    /// it fails).
    #[serde(skip)]
    Unknown {
        /// The event's `t` value.
        event_type: String,
    },
}

/// The `t` value of every event this build understands.
const KNOWN_EVENT_TYPES: [&str; 32] = [
    "messageCreate",
    "messageUpdate",
    "messageDelete",
    "channelCreate",
    "channelUpdate",
    "channelDelete",
    "guildMemberAdd",
    "guildMemberRemove",
    "guildMemberUpdate",
    "voiceStateUpdate",
    "presenceUpdate",
    "statusSettingUpdate",
    "userUpdate",
    "typingStart",
    "guildCreate",
    "guildUpdate",
    "guildRoleCreate",
    "guildRoleUpdate",
    "guildRoleDelete",
    "dmCreate",
    "dmUpdate",
    "dmDelete",
    "relationshipAdd",
    "relationshipRemove",
    "nudge",
    "readStateUpdate",
    "interactionCreate",
    "ephemeralMessage",
    "notificationSettingsUpdate",
    "guildDelete",
    "joinRequestCreate",
    "joinRequestUpdate",
];

impl Event {
    /// The event's wire type (`t`), e.g. `"messageCreate"`.
    #[must_use]
    pub fn event_type(&self) -> &str {
        match self {
            Self::MessageCreate(_) => "messageCreate",
            Self::MessageUpdate(_) => "messageUpdate",
            Self::MessageDelete(_) => "messageDelete",
            Self::ChannelCreate(_) => "channelCreate",
            Self::ChannelUpdate(_) => "channelUpdate",
            Self::ChannelDelete(_) => "channelDelete",
            Self::GuildMemberAdd(_) => "guildMemberAdd",
            Self::GuildMemberRemove(_) => "guildMemberRemove",
            Self::GuildMemberUpdate(_) => "guildMemberUpdate",
            Self::VoiceStateUpdate(_) => "voiceStateUpdate",
            Self::PresenceUpdate(_) => "presenceUpdate",
            Self::StatusSettingUpdate(_) => "statusSettingUpdate",
            Self::UserUpdate(_) => "userUpdate",
            Self::TypingStart(_) => "typingStart",
            Self::GuildCreate(_) => "guildCreate",
            Self::GuildUpdate(_) => "guildUpdate",
            Self::GuildRoleCreate(_) => "guildRoleCreate",
            Self::GuildRoleUpdate(_) => "guildRoleUpdate",
            Self::GuildRoleDelete(_) => "guildRoleDelete",
            Self::DmCreate(_) => "dmCreate",
            Self::DmUpdate(_) => "dmUpdate",
            Self::DmDelete(_) => "dmDelete",
            Self::RelationshipAdd(_) => "relationshipAdd",
            Self::RelationshipRemove(_) => "relationshipRemove",
            Self::Nudge(_) => "nudge",
            Self::ReadStateUpdate(_) => "readStateUpdate",
            Self::InteractionCreate(_) => "interactionCreate",
            Self::EphemeralMessage(_) => "ephemeralMessage",
            Self::NotificationSettingsUpdate(_) => "notificationSettingsUpdate",
            Self::GuildDelete(_) => "guildDelete",
            Self::JoinRequestCreate(_) => "joinRequestCreate",
            Self::JoinRequestUpdate(_) => "joinRequestUpdate",
            Self::Unknown { event_type } => event_type,
        }
    }
}

/// Decodes an [`Event`], mapping unknown event types to [`Event::Unknown`]
/// instead of failing. Malformed known events are still errors.
pub(super) fn deserialize_lenient<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Event, D::Error> {
    #[derive(Deserialize)]
    struct RawEvent {
        t: String,
        #[serde(default)]
        d: serde_json::Value,
    }

    let RawEvent { t, d } = RawEvent::deserialize(deserializer)?;
    if !KNOWN_EVENT_TYPES.contains(&t.as_str()) {
        return Ok(Event::Unknown { event_type: t });
    }
    let mut tagged = serde_json::Map::with_capacity(2);
    tagged.insert("t".to_owned(), serde_json::Value::String(t));
    tagged.insert("d".to_owned(), d);
    Event::deserialize(serde_json::Value::Object(tagged)).map_err(D::Error::custom)
}

/// Payload of [`Event::Nudge`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Nudge {
    /// The guild both are in.
    pub guild_id: Snowflake,
    /// Who is nudging (the guild's owner).
    pub from: User,
}

/// Payload of [`Event::MessageDelete`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageDelete {
    /// The deleted message.
    pub id: Snowflake,
    /// Its channel.
    pub channel_id: Snowflake,
}

/// Payload of [`Event::ChannelDelete`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelDelete {
    /// The deleted channel.
    pub id: Snowflake,
    /// Its guild.
    pub guild_id: Snowflake,
}

/// Payload of [`Event::GuildMemberAdd`]: the membership plus the user, so
/// clients can render the new member without another request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildMemberAdd {
    /// The new membership.
    pub member: Member,
    /// The joining user.
    pub user: User,
}

/// Payload of [`Event::GuildDelete`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildDelete {
    /// The deleted guild.
    pub guild_id: Snowflake,
}

/// Payload of [`Event::JoinRequestCreate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinRequestCount {
    /// The guild.
    pub guild_id: Snowflake,
    /// Its pending join requests now.
    pub count: u32,
}

/// Payload of [`Event::JoinRequestUpdate`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinRequestUpdate {
    /// The guild asked to join.
    pub guild_id: Snowflake,
    /// Its name, for the notice ("{Sunucu} katılma isteğini kabul etti"):
    /// the requester has no snapshot of a guild they are not in.
    pub guild_name: String,
    /// What happened: `approved`, `rejected`, `withdrawn` (on another of
    /// our devices) or `closed` (the guild stopped taking requests).
    pub status: JoinRequestStatus,
}

/// Payload of [`Event::GuildMemberRemove`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildMemberRemove {
    /// The guild.
    pub guild_id: Snowflake,
    /// The removed user.
    pub user_id: Snowflake,
}

/// Payload of [`Event::PresenceUpdate`], also the entries of
/// [`GuildSnapshot::presences`].
///
/// `version` grows with every presence change on the server, so a client
/// keeps the newest status per user whatever order snapshots and updates
/// arrive in (§8.4). A device connecting or leaving is a change too when it
/// changes `devices`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresenceUpdate {
    /// The user.
    pub user_id: Snowflake,
    /// New status.
    pub status: PresenceStatus,
    /// Ordering of presence changes; newer wins.
    #[serde(default)]
    pub version: u64,
    /// The platforms of the user's connected devices, each once, in
    /// [`Platform`] order. Empty (and omitted) while offline or invisible,
    /// for bots, and for devices whose client did not say. Kept for clients
    /// older than [`Self::devices`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub platforms: Vec<Platform>,
    /// The user's connected devices with their forms, each kind once, in
    /// [`Device`] order (computers first). Empty (and omitted) exactly when
    /// `platforms` is; older servers never send it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<Device>,
}

/// Payload of [`Event::TypingStart`]. Clients expire the indicator after a
/// few seconds unless it is repeated.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypingStart {
    /// The channel.
    pub channel_id: Snowflake,
    /// The typing user.
    pub user_id: Snowflake,
}

/// Payload of [`Event::GuildRoleDelete`]. Members lose the role implicitly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildRoleDelete {
    /// The guild.
    pub guild_id: Snowflake,
    /// The deleted role.
    pub role_id: Snowflake,
}

/// Payload of [`Event::RelationshipRemove`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipRemove {
    /// The other user.
    pub user_id: Snowflake,
}

/// Payload of [`Event::DmDelete`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DmDelete {
    /// The DM the user left.
    pub id: Snowflake,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_event_types_are_unique_real_variants() {
        let mut seen = std::collections::HashSet::new();
        for name in KNOWN_EVENT_TYPES {
            assert!(seen.insert(name), "duplicate event type {name}");
            // A real variant fails on its (empty) payload, not on the tag
            // (or takes it, when every field has a default).
            let raw = serde_json::json!({"t": name, "d": {}});
            if let Err(error) = Event::deserialize(raw) {
                let error = error.to_string();
                assert!(!error.contains("unknown variant"), "{name}: {error}");
            }
        }
    }
}
