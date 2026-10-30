//! Reports of messages, users and guilds (ADR-090,
//! `docs/contracts/account-deletion-and-reports.md` §3).
//!
//! A report stores a copy of what was reported ([`ReportSnapshot`]) taken
//! when it was made, so later edits, deletions, a purged account or a
//! deleted guild do not change the evidence. The operators see every
//! report (the operator panel); a guild's moderators (`MANAGE_MESSAGES`)
//! see the message reports of their channels, never who reported.

use serde::{Deserialize, Serialize};

use crate::snowflake::Snowflake;
use crate::timestamp::Timestamp;

/// Most characters of [`CreateReportRequest::details`] (after trimming).
pub const MAX_REPORT_DETAILS_LEN: usize = 1000;
/// Most reports one user may send per hour.
pub const REPORTS_PER_HOUR: u32 = 20;
/// Largest page of `GET /v1/guilds/{guild}/reports` (also the default).
pub const MAX_REPORT_PAGE: u32 = 50;

/// What a report is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ReportTarget {
    /// A message in a guild channel or a DM.
    Message {
        /// Its channel (or DM).
        channel_id: Snowflake,
        /// The message.
        message_id: Snowflake,
    },
    /// A user.
    User {
        /// The user.
        user_id: Snowflake,
    },
    /// A guild.
    Guild {
        /// The guild.
        guild_id: Snowflake,
    },
    /// A kind introduced after this build. Decoded only, never sent.
    #[serde(other)]
    Unknown,
}

/// Why something is reported. Clients show their own text per reason
/// (Spam; Taciz veya zorbalık; Nefret söylemi; Cinsel içerik; Çocuk
/// güvenliği; Şiddet veya tehdit; Kendine zarar verme; Yasa dışı içerik;
/// Başkasını taklit etme; Diğer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReportReason {
    /// Spam.
    Spam,
    /// Harassment or bullying.
    Harassment,
    /// Hate speech.
    Hate,
    /// Sexual content.
    Sexual,
    /// Child safety.
    ChildSafety,
    /// Violence or threats.
    Violence,
    /// Self-harm.
    SelfHarm,
    /// Illegal content.
    Illegal,
    /// Impersonating someone.
    Impersonation,
    /// Anything else (say what in the details).
    Other,
    /// A reason introduced after this build. Decoded only; the server
    /// refuses it.
    #[serde(other)]
    Unknown,
}

impl ReportReason {
    /// Every reason a report can have, in the order clients list them.
    pub const ALL: [Self; 10] = [
        Self::Spam,
        Self::Harassment,
        Self::Hate,
        Self::Sexual,
        Self::ChildSafety,
        Self::Violence,
        Self::SelfHarm,
        Self::Illegal,
        Self::Impersonation,
        Self::Other,
    ];

    /// The wire name, e.g. `childSafety`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Spam => "spam",
            Self::Harassment => "harassment",
            Self::Hate => "hate",
            Self::Sexual => "sexual",
            Self::ChildSafety => "childSafety",
            Self::Violence => "violence",
            Self::SelfHarm => "selfHarm",
            Self::Illegal => "illegal",
            Self::Impersonation => "impersonation",
            Self::Other => "other",
            Self::Unknown => "unknown",
        }
    }

    /// The reason with this wire name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|reason| reason.as_str() == name)
    }
}

/// Where a report stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReportStatus {
    /// Nobody has dealt with it yet.
    Open,
    /// Someone acted on it.
    Resolved,
    /// Someone found nothing to act on.
    Dismissed,
    /// A status introduced after this build. Decoded only.
    #[serde(other)]
    Unknown,
}

impl ReportStatus {
    /// The wire name (and the stored value).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Resolved => "resolved",
            Self::Dismissed => "dismissed",
            Self::Unknown => "unknown",
        }
    }

    /// The status with this wire name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Open, Self::Resolved, Self::Dismissed]
            .into_iter()
            .find(|status| status.as_str() == name)
    }
}

/// `POST /v1/reports` (people only) → [`ReportCreated`]. The caller must
/// see the target (a message in a channel they can read or a DM they are
/// in; a user they share a guild, a DM or a relationship with; a guild they
/// are a member of), else `404`; never themselves, their own message or
/// their own guild (`400`). At most [`REPORTS_PER_HOUR`] an hour (`429`).
/// The same reporter's open report on the same target is answered again
/// instead of a new one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateReportRequest {
    /// What is reported.
    pub target: ReportTarget,
    /// Why.
    pub reason: ReportReason,
    /// Optional free text, trimmed, at most [`MAX_REPORT_DETAILS_LEN`]
    /// characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// The answer to `POST /v1/reports`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportCreated {
    /// The report (an existing one for a repeated report).
    pub id: Snowflake,
}

/// A file of a reported message, as it was.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportedAttachment {
    /// The attachment.
    pub id: Snowflake,
    /// Its file name.
    pub filename: String,
    /// Its MIME type.
    pub content_type: String,
    /// Its size in bytes.
    pub size: u64,
}

/// The evidence copy of a report, taken when it was made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ReportSnapshot {
    /// A message.
    Message {
        /// The message.
        message_id: Snowflake,
        /// Its text.
        content: String,
        /// Its author.
        author_id: Snowflake,
        /// The author's username then.
        author_username: String,
        /// The author's display name then.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        author_display_name: Option<String>,
        /// Its files.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        attachments: Vec<ReportedAttachment>,
        /// When it was sent.
        created_at: Timestamp,
        /// When it was last edited.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        edited_at: Option<Timestamp>,
        /// Its channel (or DM).
        channel_id: Snowflake,
        /// The channel's name; `None` in a DM.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        channel_name: Option<String>,
        /// The guild; `None` in a DM.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        guild_id: Option<Snowflake>,
        /// The guild's name; `None` in a DM.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        guild_name: Option<String>,
    },
    /// A user's profile.
    User {
        /// The user.
        user_id: Snowflake,
        /// Their username then.
        username: String,
        /// Their display name then.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        display_name: Option<String>,
        /// Their bio then.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bio: Option<String>,
        /// Their avatar hash then.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        avatar: Option<String>,
    },
    /// A guild.
    Guild {
        /// The guild.
        guild_id: Snowflake,
        /// Its name then.
        name: String,
        /// Its icon hash then.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
        /// Its owner then.
        owner_id: Snowflake,
        /// How many members it had.
        member_count: u32,
    },
    /// A kind introduced after this build. Decoded only.
    #[serde(other)]
    Unknown,
}

/// `GET /v1/guilds/{guild}/reports` query: newest first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildReportsQuery {
    /// Only reports with this status; all when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ReportStatus>,
    /// Only reports older than this one (the last ID of the previous page).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<Snowflake>,
    /// Page size, 1 to [`MAX_REPORT_PAGE`] (the default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// A message report as a guild's moderators see it: everything but who
/// reported (moderators must not be able to retaliate).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildReport {
    /// The report.
    pub id: Snowflake,
    /// What was reported (the message).
    pub target: ReportTarget,
    /// Why.
    pub reason: ReportReason,
    /// The reporter's text, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    /// The message as it was when reported.
    pub snapshot: ReportSnapshot,
    /// Where it stands.
    pub status: ReportStatus,
    /// When it was made.
    pub created_at: Timestamp,
    /// When it was resolved or dismissed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<Timestamp>,
}

/// `PATCH /v1/guilds/{guild}/reports/{report}` (`MANAGE_MESSAGES` in the
/// reported message's channel) → 204.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReportRequest {
    /// `resolved` or `dismissed` (`open` reopens it).
    pub status: ReportStatus,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn targets_and_reasons_have_their_wire_shapes() {
        let target = ReportTarget::Message {
            channel_id: Snowflake::new(1),
            message_id: Snowflake::new(2),
        };
        assert_eq!(
            serde_json::to_value(target).unwrap(),
            json!({"kind": "message", "channelId": "1", "messageId": "2"})
        );
        let later: ReportTarget = serde_json::from_value(json!({"kind": "voiceRoom"})).unwrap();
        assert_eq!(later, ReportTarget::Unknown);
        for reason in ReportReason::ALL {
            let json = serde_json::to_value(reason).unwrap();
            assert_eq!(json, reason.as_str());
            assert_eq!(ReportReason::from_name(reason.as_str()), Some(reason));
        }
        let later: ReportReason = serde_json::from_value(json!("scam")).unwrap();
        assert_eq!(later, ReportReason::Unknown);
        assert_eq!(ReportReason::from_name("unknown"), None);
        assert_eq!(ReportStatus::from_name("open"), Some(ReportStatus::Open));
        assert_eq!(ReportStatus::from_name("unknown"), None);
    }

    #[test]
    fn snapshots_round_trip() {
        let snapshot = ReportSnapshot::Guild {
            guild_id: Snowflake::new(5),
            name: "Kuzenler".into(),
            icon: None,
            owner_id: Snowflake::new(6),
            member_count: 3,
        };
        let json = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(json["kind"], "guild");
        assert_eq!(json["memberCount"], 3);
        assert_eq!(
            serde_json::from_value::<ReportSnapshot>(json).unwrap(),
            snapshot
        );
    }
}
