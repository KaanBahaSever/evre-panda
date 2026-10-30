//! Profile and guild images (avatars, banners, guild icons).
//!
//! An image is uploaded as a raw body with `PUT` and identified afterwards
//! by a content hash ([`User::avatar`](crate::User::avatar) and friends).
//! Its public URL is `{base}/v1/media/{kind}/{owner}/{hash}`
//! ([`media_path`]): no auth, immutable, so clients and WebViews may cache
//! it forever. A new image gets a new hash and so a new URL; the old one
//! stops working.
//!
//! Uploads need `Content-Length` (`411` otherwise) and are checked before
//! anything is kept: at most [`MediaKind::max_bytes`], one of
//! [`IMAGE_TYPES`] recognized from the first bytes (the `Content-Type`
//! header is not trusted), at most
//! [`MAX_IMAGE_SIDE`](limits::MAX_IMAGE_SIDE) pixels wide and high.
//! Anything else is a `400` validation error on `body`. `DELETE` on the
//! same route removes the image.

use std::fmt;
use std::str::FromStr;

use super::limits;
use crate::snowflake::Snowflake;

/// Image types accepted for uploads (detected from the file's first bytes).
pub const IMAGE_TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/gif", "image/webp"];

/// Length of a media hash: the first hex characters of the image's SHA-256.
pub const MEDIA_HASH_LEN: usize = 16;

/// What an image is for; also the `{kind}` segment of its URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaKind {
    /// A user's avatar (`avatars`).
    Avatar,
    /// A user's profile banner (`banners`).
    Banner,
    /// A guild's icon (`icons`).
    GuildIcon,
    /// A guild's banner (`guild-banners`).
    GuildBanner,
}

impl MediaKind {
    /// Every kind.
    pub const ALL: [Self; 4] = [
        Self::Avatar,
        Self::Banner,
        Self::GuildIcon,
        Self::GuildBanner,
    ];

    /// The URL segment, e.g. `"avatars"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Avatar => "avatars",
            Self::Banner => "banners",
            Self::GuildIcon => "icons",
            Self::GuildBanner => "guild-banners",
        }
    }

    /// Largest accepted upload in bytes:
    /// [`MAX_AVATAR_BYTES`](limits::MAX_AVATAR_BYTES) for avatars and icons,
    /// [`MAX_BANNER_BYTES`](limits::MAX_BANNER_BYTES) for banners.
    #[must_use]
    pub const fn max_bytes(self) -> u64 {
        match self {
            Self::Avatar | Self::GuildIcon => limits::MAX_AVATAR_BYTES,
            Self::Banner | Self::GuildBanner => limits::MAX_BANNER_BYTES,
        }
    }
}

impl fmt::Display for MediaKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A `{kind}` segment that names no [`MediaKind`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown media kind")]
pub struct UnknownMediaKind;

impl FromStr for MediaKind {
    type Err = UnknownMediaKind;

    fn from_str(segment: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == segment)
            .ok_or(UnknownMediaKind)
    }
}

/// Whether `hash` has the form of a media hash: [`MEDIA_HASH_LEN`]
/// lowercase hex characters.
#[must_use]
pub fn is_media_hash(hash: &str) -> bool {
    hash.len() == MEDIA_HASH_LEN && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The path of an image relative to the server's base URL, e.g.
/// `/v1/media/avatars/42/0123456789abcdef`. `owner` is the user for
/// avatars and banners, the guild for icons and guild banners.
#[must_use]
pub fn media_path(kind: MediaKind, owner: Snowflake, hash: &str) -> String {
    format!("/v1/media/{kind}/{owner}/{hash}")
}
