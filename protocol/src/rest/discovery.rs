//! Who may join a guild, Keşfet and join requests (ADR-095,
//! `docs/contracts/guild-access-and-discovery.md`).
//!
//! A guild's [`GuildAccess`] decides how people get in: with an invite
//! only (`inviteOnly`, the default), not at all (`closed`), freely from
//! Keşfet (`public`) or by a join request a moderator approves
//! (`request`). Listed guilds (`public`, `request`) appear in
//! `GET /v1/discovery/guilds` as [`DiscoveryGuild`] cards, never to someone
//! they ban and never while an operator hid them.

use serde::{Deserialize, Serialize};

use crate::locale::Locale;
use crate::model::{GuildAccess, GuildCategory, User};
use crate::rest::{MediaKind, media_path};
use crate::snowflake::Snowflake;
use crate::timestamp::Timestamp;

/// Most characters of a guild's Keşfet description (after trimming).
pub const MAX_GUILD_DESCRIPTION_LEN: usize = 300;
/// Most characters of a join request's message (after trimming).
pub const MAX_JOIN_REQUEST_MESSAGE_LEN: usize = 300;
/// Largest page of `GET /v1/discovery/guilds` (also the default).
pub const MAX_DISCOVERY_PAGE: u32 = 50;
/// Largest page of `GET /v1/guilds/{guild}/join-requests` (also the
/// default).
pub const MAX_JOIN_REQUEST_PAGE: u32 = 50;
/// Keşfet requests (the list and previews) per user and minute.
pub const DISCOVERY_PER_MINUTE: u32 = 60;
/// Join requests one user may send per hour.
pub const JOIN_REQUESTS_PER_HOUR: u32 = 10;
/// Hours after a rejection before the same user may ask the same guild
/// again.
pub const JOIN_REQUEST_COOLDOWN_HOURS: u64 = 24;
/// Days a decided (or withdrawn) request is kept; the audit rows stay.
pub const JOIN_REQUEST_RETENTION_DAYS: u32 = 30;

/// The order of Keşfet.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiscoverySort {
    /// Most online members first, then most members (the default).
    #[default]
    Online,
    /// Most members first, then most online.
    Members,
    /// Newest guilds first.
    New,
    /// A sort introduced after this build. Decoded only; the server
    /// refuses it.
    #[serde(other)]
    Unknown,
}

impl DiscoverySort {
    /// The wire name, e.g. `online`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Members => "members",
            Self::New => "new",
            Self::Unknown => "unknown",
        }
    }
}

/// Query of `GET /v1/discovery/guilds` (signed in; people and bots alike).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryQuery {
    /// Words to find in the name or description, all of them (case,
    /// accents and the Turkish dotted and dotless i do not matter).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    /// Only this category.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<GuildCategory>,
    /// Only guilds in this language.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<Locale>,
    /// The order; default [`DiscoverySort::Online`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<DiscoverySort>,
    /// [`DiscoveryPage::next_cursor`] of the page before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// At most [`MAX_DISCOVERY_PAGE`] (the default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// A Keşfet card, and `GET /v1/guilds/{guild}/preview`: the listing
/// fields and where the caller stands. Never channels or messages.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryGuild {
    /// The guild.
    pub id: Snowflake,
    /// Its name.
    pub name: String,
    /// Content hash of its icon ([`Self::icon_path`]).
    #[serde(default)]
    pub icon: Option<String>,
    /// Content hash of its banner ([`Self::banner_path`]).
    #[serde(default)]
    pub banner: Option<String>,
    /// Its Keşfet description.
    #[serde(default)]
    pub description: Option<String>,
    /// Its category.
    #[serde(default)]
    pub category: Option<GuildCategory>,
    /// Its language.
    #[serde(default)]
    pub locale: Option<Locale>,
    /// How to join: **Katıl** for `public`, **Katılma isteği gönder** for
    /// `request` (a preview of a guild the caller is in may show the other
    /// modes).
    pub access: GuildAccess,
    /// Members (bots included, as in invite previews).
    pub member_count: u64,
    /// Members who are online now.
    pub online_count: u64,
    /// When the guild was created.
    pub created_at: Timestamp,
    /// The caller is a member already (**Açık**).
    #[serde(default)]
    pub member: bool,
    /// The caller has a pending join request (**İstek gönderildi**).
    #[serde(default)]
    pub requested: bool,
    /// The caller's last request was rejected: no new one before this time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejected_until: Option<Timestamp>,
}

impl DiscoveryGuild {
    /// Path of the icon image relative to the server's base URL.
    #[must_use]
    pub fn icon_path(&self) -> Option<String> {
        let hash = self.icon.as_deref()?;
        Some(media_path(MediaKind::GuildIcon, self.id, hash))
    }

    /// Path of the banner image relative to the server's base URL.
    #[must_use]
    pub fn banner_path(&self) -> Option<String> {
        let hash = self.banner.as_deref()?;
        Some(media_path(MediaKind::GuildBanner, self.id, hash))
    }
}

/// The answer to `GET /v1/discovery/guilds`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryPage {
    /// The cards, in the requested order.
    pub guilds: Vec<DiscoveryGuild>,
    /// Pass as `cursor` for the next page; `None` on the last one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Where a join request stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JoinRequestStatus {
    /// Waiting for a moderator.
    Pending,
    /// A moderator let the requester in (or the guild became `public`).
    Approved,
    /// A moderator said no: no new request for
    /// [`JOIN_REQUEST_COOLDOWN_HOURS`].
    Rejected,
    /// The requester took it back.
    Withdrawn,
    /// The guild stopped taking requests (it became `inviteOnly` or
    /// `closed`).
    Closed,
    /// A status introduced after this build. Decoded only.
    #[serde(other)]
    Unknown,
}

impl JoinRequestStatus {
    /// The wire name (and the stored value).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Withdrawn => "withdrawn",
            Self::Closed => "closed",
            Self::Unknown => "unknown",
        }
    }

    /// The status with this wire name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        [
            Self::Pending,
            Self::Approved,
            Self::Rejected,
            Self::Withdrawn,
            Self::Closed,
        ]
        .into_iter()
        .find(|status| status.as_str() == name)
    }
}

/// `POST /v1/guilds/{guild}/join-requests` (people only; a `request`
/// guild that does not ban the caller) → [`JoinRequest`]. A pending
/// request answers again; `429` with `retryAfterMs` while the rejection
/// cooldown runs or beyond [`JOIN_REQUESTS_PER_HOUR`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateJoinRequest {
    /// Optional words for the moderators, trimmed, at most
    /// [`MAX_JOIN_REQUEST_MESSAGE_LEN`] characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// The caller's own join request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinRequest {
    /// The guild asked.
    pub guild_id: Snowflake,
    /// Where it stands (`pending` after `POST`).
    pub status: JoinRequestStatus,
    /// The message sent with it.
    #[serde(default)]
    pub message: Option<String>,
    /// When it was sent.
    pub created_at: Timestamp,
}

/// A pending join request as the guild's moderators (`KICK_MEMBERS`) see
/// it in `GET /v1/guilds/{guild}/join-requests` (oldest first).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildJoinRequest {
    /// The request; pass the last one as `cursor` for the next page.
    pub id: Snowflake,
    /// Who asks.
    pub user_id: Snowflake,
    /// Their profile.
    pub user: User,
    /// Their message.
    #[serde(default)]
    pub message: Option<String>,
    /// When they asked.
    pub created_at: Timestamp,
    /// When their account was made (its age).
    pub user_created_at: Timestamp,
    /// Guilds the moderator shares with them.
    pub mutual_guilds: u32,
}

/// Query of `GET /v1/guilds/{guild}/join-requests`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinRequestsQuery {
    /// The last [`GuildJoinRequest::id`] of the page before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<Snowflake>,
    /// At most [`MAX_JOIN_REQUEST_PAGE`] (the default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// `POST /v1/guilds/{guild}/join-requests/{user}/reject` (`KICK_MEMBERS`);
/// the body is optional. The reason is for the moderators' records (the
/// audit log), like a ban's: the requester only learns the rejection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectJoinRequest {
    /// Why, one line, at most
    /// [`MAX_JOIN_REQUEST_MESSAGE_LEN`] characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_round_trip_by_name() {
        for status in [
            JoinRequestStatus::Pending,
            JoinRequestStatus::Approved,
            JoinRequestStatus::Rejected,
            JoinRequestStatus::Withdrawn,
            JoinRequestStatus::Closed,
        ] {
            assert_eq!(JoinRequestStatus::from_name(status.as_str()), Some(status));
            assert_eq!(
                serde_json::to_value(status).unwrap(),
                serde_json::json!(status.as_str())
            );
        }
        let later: JoinRequestStatus = serde_json::from_value(serde_json::json!("later")).unwrap();
        assert_eq!(later, JoinRequestStatus::Unknown);
    }

    #[test]
    fn a_card_without_the_callers_state_decodes() {
        let card: DiscoveryGuild = serde_json::from_value(serde_json::json!({
            "id": "5", "name": "Kuzenler", "access": "request",
            "memberCount": 3, "onlineCount": 1, "createdAt": 1000
        }))
        .unwrap();
        assert_eq!(card.access, GuildAccess::Request);
        assert!(!card.member && !card.requested);
        assert_eq!(card.rejected_until, None);
        let json = serde_json::to_value(&card).unwrap();
        assert!(json.get("rejectedUntil").is_none());
    }
}
