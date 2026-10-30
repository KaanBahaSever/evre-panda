use entitlements::PlanId;
use serde::{Deserialize, Serialize};

use crate::locale::Locale;
use crate::rest::{MediaKind, media_path};
use crate::snowflake::Snowflake;

/// Who may join a guild (ADR-095,
/// `docs/contracts/guild-access-and-discovery.md` §1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GuildAccess {
    /// "Davetle katılım": only with an invite (the default, and every
    /// guild from before ADR-095).
    #[default]
    InviteOnly,
    /// "Katılıma kapalı": nobody new joins, invites stop working
    /// ([`ApiError::GuildClosed`](crate::ApiError::GuildClosed)); members
    /// stay.
    Closed,
    /// "Herkese açık": listed in Keşfet, anyone signed in joins; invites
    /// work too.
    Public,
    /// "Herkese açık, istekle": listed in Keşfet, joined by a request a
    /// moderator (`KICK_MEMBERS`) approves; invites still join directly.
    Request,
    /// A mode introduced after this build. Decoded only; the server
    /// refuses it.
    #[serde(other)]
    Unknown,
}

impl GuildAccess {
    /// Every mode, in the order the settings list them.
    pub const ALL: [Self; 4] = [Self::InviteOnly, Self::Closed, Self::Public, Self::Request];

    /// The wire name (and the stored value), e.g. `inviteOnly`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InviteOnly => "inviteOnly",
            Self::Closed => "closed",
            Self::Public => "public",
            Self::Request => "request",
            Self::Unknown => "unknown",
        }
    }

    /// The mode with this wire name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|access| access.as_str() == name)
    }

    /// Listed in Keşfet (`public` and `request`); such a guild needs a
    /// description and a category.
    #[must_use]
    pub const fn is_listed(self) -> bool {
        matches!(self, Self::Public | Self::Request)
    }
}

/// What a listed guild is about (Keşfet's category chips).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GuildCategory {
    /// Oyun.
    Gaming,
    /// Müzik.
    Music,
    /// Sohbet.
    Chat,
    /// Eğitim.
    Education,
    /// Teknoloji.
    Tech,
    /// Sanat.
    Art,
    /// Spor.
    Sports,
    /// Diğer.
    Other,
    /// A category introduced after this build. Decoded only; the server
    /// refuses it.
    #[serde(other)]
    Unknown,
}

impl GuildCategory {
    /// Every category, in the order clients list them.
    pub const ALL: [Self; 8] = [
        Self::Gaming,
        Self::Music,
        Self::Chat,
        Self::Education,
        Self::Tech,
        Self::Art,
        Self::Sports,
        Self::Other,
    ];

    /// The wire name (and the stored value), e.g. `gaming`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gaming => "gaming",
            Self::Music => "music",
            Self::Chat => "chat",
            Self::Education => "education",
            Self::Tech => "tech",
            Self::Art => "art",
            Self::Sports => "sports",
            Self::Other => "other",
            Self::Unknown => "unknown",
        }
    }

    /// The category with this wire name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|category| category.as_str() == name)
    }
}

/// A guild ("server"): the tenant that owns channels, roles and a plan.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Guild {
    /// Guild ID. Also the ID of its `@everyone` role.
    pub id: Snowflake,
    /// Display name.
    pub name: String,
    /// The owner, who bypasses every permission check.
    pub owner_id: Snowflake,
    /// The guild's subscription plan.
    pub plan: PlanId,
    /// Content hash of the guild icon ([`Self::icon_path`]); `None` shows
    /// the name's initials.
    pub icon: Option<String>,
    /// Content hash of the guild banner ([`Self::banner_path`]).
    pub banner: Option<String>,
    /// Who may join (ADR-095). Older servers never send it: their guilds
    /// are [`GuildAccess::InviteOnly`].
    #[serde(default)]
    pub access: GuildAccess,
    /// The Keşfet description (at most
    /// [`MAX_GUILD_DESCRIPTION_LEN`](crate::rest::MAX_GUILD_DESCRIPTION_LEN)
    /// characters, trimmed); required while the guild is listed
    /// ([`GuildAccess::is_listed`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The Keşfet category; required with the description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<GuildCategory>,
    /// The guild's language in Keşfet; set to the owner's UI language
    /// when the guild is first listed without one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<Locale>,
}

impl Guild {
    /// The ID of the guild's `@everyone` role, which equals the guild ID.
    #[must_use]
    pub const fn everyone_role_id(&self) -> Snowflake {
        self.id
    }

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
