//! Push notifications for the mobile apps (architecture.md §8.1
//! "Notifications"): a phone registers its push token for its login
//! session, and the server sends small data messages through the
//! provider (Firebase Cloud Messaging now, Apple's APNs later) when
//! something wants the user's attention while the app may not be running.
//!
//! Pushes carry what happened and where ([`PushData`]) plus the title,
//! the text to show and the `evre://` link
//! (`docs/contracts/notifications.md` §3). The title names the community
//! and channel or the author; the start of the message travels through the
//! provider only when the user's synced `previews` switch is on
//! ([`NotificationSettings`](super::NotificationSettings)), otherwise the
//! text is "Yeni mesaj".

use serde::{Deserialize, Serialize};

use crate::snowflake::Snowflake;

/// Longest accepted device token (FCM registration tokens are ~160
/// characters, APNs device tokens 64 hex digits).
pub const MAX_PUSH_TOKEN_LEN: usize = 4096;

/// A push service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PushProvider {
    /// Firebase Cloud Messaging (Android).
    Fcm,
    /// Apple Push Notification service (iOS, later).
    Apns,
    /// A provider this build does not know.
    #[serde(other)]
    Unknown,
}

/// What a device wants pushed. Every field has a default, so older and
/// newer apps can send fewer or more of them. Since the synced
/// notification settings these are per-device opt-outs on top of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushSettings {
    /// Direct messages.
    #[serde(default = "yes")]
    pub dms: bool,
    /// Mentions in guild channels, pinging replies included.
    #[serde(default = "yes")]
    pub mentions: bool,
    /// Incoming friend requests.
    #[serde(default = "yes")]
    pub friend_requests: bool,
    /// Ignored since the synced settings (2026-10-28): previews follow
    /// `NotificationSettings.global.previews` on every device.
    #[serde(default)]
    pub previews: bool,
}

const fn yes() -> bool {
    true
}

impl Default for PushSettings {
    fn default() -> Self {
        Self {
            dms: true,
            mentions: true,
            friend_requests: true,
            previews: false,
        }
    }
}

/// `PUT /v1/users/@me/push`: registers (or updates) the push token of the
/// calling login session. One registration per login session: signing out
/// ends it, and a token registered again by another session (another
/// account on the same phone) moves there. People only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterPushRequest {
    /// Which service the token belongs to.
    pub provider: PushProvider,
    /// The provider's device token (1 to [`MAX_PUSH_TOKEN_LEN`] printable
    /// ASCII characters).
    pub token: String,
    /// What to push.
    #[serde(default)]
    pub settings: PushSettings,
    /// The version of the Açık Rıza Metni whose push consent the person
    /// gave on this device ([`legal::CONSENT_VERSION`](super::legal::CONSENT_VERSION)).
    /// Required since 2026-10-28 (ADR-087): without it the server stores
    /// no token, so a build that never asked sends no pushes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consent_version: Option<String>,
}

/// The answer of `PUT /v1/users/@me/push`: what is stored for the session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushRegistration {
    /// The token's service.
    pub provider: PushProvider,
    /// What is pushed.
    pub settings: PushSettings,
    /// The server can deliver through this provider now; `false`: the
    /// token is stored, but no sender is configured (yet).
    pub enabled: bool,
}

/// What a push is about ([`PushData::kind`], also its `type`; contract
/// `docs/contracts/notifications.md` §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PushKind {
    /// A message in a DM we take part in.
    Dm,
    /// A guild message that mentions us.
    Mention,
    /// Someone sent us a friend request ([`PushData::user_id`]).
    FriendRequest,
    /// We read [`PushData::channel_id`] on another device: remove its
    /// notification. Never shown.
    Read,
    /// A guild message at the level `all`.
    Message,
    /// A reply to one of our messages that pings us.
    Reply,
    /// The guild owner wakes us up ("Uyandır").
    Nudge,
    /// A kind this build does not know: ignore it.
    #[serde(other)]
    Unknown,
}

impl From<crate::rest::NotificationKind> for PushKind {
    fn from(kind: crate::rest::NotificationKind) -> Self {
        use crate::rest::NotificationKind as K;
        match kind {
            K::Message => Self::Message,
            K::Mention => Self::Mention,
            K::Reply => Self::Reply,
            K::Dm => Self::Dm,
            K::FriendRequest => Self::FriendRequest,
            K::Nudge => Self::Nudge,
        }
    }
}

/// Version of [`PushData`] sent today.
pub const PUSH_DATA_VERSION: &str = "1";

/// The data of one push. On the wire every value is a string (FCM's `data`
/// map; APNs custom keys), absent fields are left out, IDs are decimal
/// strings as everywhere.
///
/// The contract's payload (`docs/contracts/notifications.md` §3) is
/// `type`, `guildId`, `channelId`, `messageId`, `authorId`, `title`,
/// `body`, `collapseKey` and `deepLink`; `kind` (the same as `type`),
/// `userId` and the preview fields `author`, `channel`, `guild` and
/// `attachments` stay for the first Android build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushData {
    /// [`PUSH_DATA_VERSION`].
    pub v: String,
    /// What happened.
    pub kind: PushKind,
    /// The same as `kind` (the contract's name).
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub push_type: Option<PushKind>,
    /// The channel or DM ([`PushKind::Dm`], [`PushKind::Mention`],
    /// [`PushKind::Read`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<Snowflake>,
    /// The channel's guild ([`PushKind::Mention`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<Snowflake>,
    /// The message ([`PushKind::Dm`], [`PushKind::Mention`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<Snowflake>,
    /// Who sent it, or who asked to be friends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<Snowflake>,
    /// The same as `userId` (the contract's name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_id: Option<Snowflake>,
    /// What to show as the title: `"{Topluluk} · #{kanal}"`, the author's
    /// name for DMs (always sent, previews or not).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// What to show as the text: `"{yazar}: {mesaj}"`, or `"Yeni mesaj"`
    /// with previews off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// Pushes with the same key replace each other on the device (Android
    /// notification tag, iOS `apns-collapse-id` and thread): the channel ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collapse_key: Option<String>,
    /// Where tapping it goes (`evre://…`, [`crate::deeplink`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deep_link: Option<String>,
    /// Previews only: the sender's shown name (guild nickname for
    /// mentions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Previews only: the channel's name, or a group DM's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// Previews only: the guild's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild: Option<String>,
    /// Previews only: how many files the message has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<String>,
    /// The user's sound switch: `"0"` asks the app to show it silently.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<String>,
}

impl PushData {
    /// A push of `kind` with nothing else set.
    #[must_use]
    pub fn new(kind: PushKind) -> Self {
        Self {
            v: PUSH_DATA_VERSION.to_owned(),
            kind,
            push_type: Some(kind),
            channel_id: None,
            guild_id: None,
            message_id: None,
            user_id: None,
            author_id: None,
            title: None,
            body: None,
            collapse_key: None,
            deep_link: None,
            author: None,
            channel: None,
            guild: None,
            attachments: None,
            sound: None,
        }
    }

    /// The data as the string map providers send (`{"v":"1","kind":"dm",…}`).
    #[must_use]
    pub fn to_map(&self) -> std::collections::BTreeMap<String, String> {
        let Ok(serde_json::Value::Object(fields)) = serde_json::to_value(self) else {
            return std::collections::BTreeMap::new();
        };
        fields
            .into_iter()
            .filter_map(|(key, value)| match value {
                serde_json::Value::String(text) => Some((key, text)),
                serde_json::Value::Null => None,
                other => Some((key, other.to_string())),
            })
            .collect()
    }

    /// Reads a provider's string map back (the app's side). Unknown keys
    /// are ignored; `None` if required fields are missing or malformed.
    #[must_use]
    pub fn from_map<'a>(map: impl IntoIterator<Item = (&'a str, &'a str)>) -> Option<Self> {
        let object: serde_json::Map<String, serde_json::Value> = map
            .into_iter()
            .map(|(key, value)| (key.to_owned(), serde_json::Value::String(value.to_owned())))
            .collect();
        serde_json::from_value(serde_json::Value::Object(object)).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_default_to_everything_without_previews() {
        let settings: PushSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, PushSettings::default());
        assert!(settings.dms && settings.mentions && settings.friend_requests);
        assert!(!settings.previews);
        let request: RegisterPushRequest =
            serde_json::from_str(r#"{"provider":"fcm","token":"abc"}"#).unwrap();
        assert_eq!(request.provider, PushProvider::Fcm);
        assert_eq!(request.settings, PushSettings::default());
        let later: RegisterPushRequest =
            serde_json::from_str(r#"{"provider":"webPush","token":"x"}"#).unwrap();
        assert_eq!(later.provider, PushProvider::Unknown);
    }

    #[test]
    fn data_is_a_map_of_strings() {
        let mut data = PushData::new(PushKind::Mention);
        data.channel_id = Some(Snowflake::new(10));
        data.guild_id = Some(Snowflake::new(20));
        data.message_id = Some(Snowflake::new(30));
        data.user_id = Some(Snowflake::new(40));
        let map = data.to_map();
        assert_eq!(map["v"], "1");
        assert_eq!(map["kind"], "mention");
        assert_eq!(map["type"], "mention");
        assert_eq!(map["channelId"], "10");
        assert_eq!(map["guildId"], "20");
        assert_eq!(map["messageId"], "30");
        assert_eq!(map["userId"], "40");
        assert!(!map.contains_key("body"), "no previews unless asked for");
        let back = PushData::from_map(map.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        assert_eq!(back, Some(data));
        let unknown = PushData::from_map([("v", "2"), ("kind", "call"), ("extra", "x")]).unwrap();
        assert_eq!(unknown.kind, PushKind::Unknown);
        // The first Android build's pushes had no `type`.
        let old = PushData::from_map([("v", "1"), ("kind", "dm"), ("channelId", "5")]).unwrap();
        assert_eq!(old.push_type, None);
        assert_eq!(old.kind, PushKind::Dm);
        assert_eq!(PushData::from_map([("v", "1")]), None);
    }
}
