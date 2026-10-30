use serde::{Deserialize, Serialize};

use crate::rest::{MediaKind, media_path};
use crate::snowflake::Snowflake;

/// A global user account (people and bots alike).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    /// User ID.
    pub id: Snowflake,
    /// Unique handle, used for login and mentions.
    pub username: String,
    /// Name shown in the UI; `None` falls back to `username`.
    pub display_name: Option<String>,
    /// Content hash of the current avatar ([`Self::avatar_path`]); `None`
    /// means the default avatar.
    pub avatar: Option<String>,
    /// Content hash of the profile banner ([`Self::banner_path`]).
    pub banner: Option<String>,
    /// Short "about me" text, at most
    /// [`MAX_BIO_LEN`](crate::rest::limits::MAX_BIO_LEN) characters.
    pub bio: Option<String>,
    /// Whether this is a customer-run bot account (§4.4).
    #[serde(default)]
    pub bot: bool,
    /// The account was deleted and purged (ADR-090): the username is a
    /// reserved placeholder and the profile is empty. Clients show the
    /// localized "Silinmiş kullanıcı" ("Deleted user") and the default
    /// avatar, and offer no DM, friend request or mention for it. Older
    /// servers never send it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
}

impl User {
    /// The name to show: the display name if set, else the username.
    #[must_use]
    pub fn shown_name(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.username)
    }

    /// Path of the avatar image relative to the server's base URL.
    #[must_use]
    pub fn avatar_path(&self) -> Option<String> {
        let hash = self.avatar.as_deref()?;
        Some(media_path(MediaKind::Avatar, self.id, hash))
    }

    /// Path of the banner image relative to the server's base URL.
    #[must_use]
    pub fn banner_path(&self) -> Option<String> {
        let hash = self.banner.as_deref()?;
        Some(media_path(MediaKind::Banner, self.id, hash))
    }
}
