//! `evre://` links (`docs/contracts/notifications.md` §3): where a
//! notification, a push or a link from anywhere takes the user.
//!
//! - `evre://channel/{guildId}/{channelId}?message={messageId}`
//! - `evre://dm/{channelId}?message={messageId}`
//! - `evre://friends/requests`
//!
//! `?message=` is optional. Parsing ignores the scheme's case, a trailing
//! `/`, a `#fragment` and unknown query parameters; IDs must be decimal
//! snowflakes. Built links are always in the form above.

use std::fmt;
use std::str::FromStr;

use crate::snowflake::Snowflake;

/// The URL scheme the apps register.
pub const SCHEME: &str = "evre";

/// Longest link accepted.
pub const MAX_LEN: usize = 512;

/// A place in the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeepLink {
    /// A guild channel, optionally at a message.
    Channel {
        /// The guild.
        guild_id: Snowflake,
        /// The channel.
        channel_id: Snowflake,
        /// The message to jump to.
        message_id: Option<Snowflake>,
    },
    /// A DM or group DM, optionally at a message.
    Dm {
        /// The DM channel.
        channel_id: Snowflake,
        /// The message to jump to.
        message_id: Option<Snowflake>,
    },
    /// The pending friend requests.
    FriendRequests,
}

impl DeepLink {
    /// Parses a link; `None` if it is not one of ours.
    #[must_use]
    pub fn parse(url: &str) -> Option<Self> {
        let url = url.trim();
        if url.len() > MAX_LEN {
            return None;
        }
        let (scheme, rest) = url.split_once("://")?;
        if !scheme.eq_ignore_ascii_case(SCHEME) {
            return None;
        }
        let rest = rest.split('#').next().unwrap_or_default();
        let (path, query) = match rest.split_once('?') {
            Some((path, query)) => (path, Some(query)),
            None => (rest, None),
        };
        let parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();
        let message_id = match query {
            None => None,
            Some(query) => {
                let mut found = None;
                for pair in query.split('&') {
                    if let Some(value) = pair.strip_prefix("message=") {
                        found = Some(id(value)?);
                    }
                }
                found
            }
        };
        match parts.as_slice() {
            ["channel", guild, channel] => Some(Self::Channel {
                guild_id: id(guild)?,
                channel_id: id(channel)?,
                message_id,
            }),
            ["dm", channel] => Some(Self::Dm {
                channel_id: id(channel)?,
                message_id,
            }),
            ["friends", "requests"] => Some(Self::FriendRequests),
            _ => None,
        }
    }

    /// The channel it opens, if any.
    #[must_use]
    pub const fn channel_id(&self) -> Option<Snowflake> {
        match self {
            Self::Channel { channel_id, .. } | Self::Dm { channel_id, .. } => Some(*channel_id),
            Self::FriendRequests => None,
        }
    }
}

/// A decimal snowflake, nothing else (no sign, no spaces, not zero).
fn id(text: &str) -> Option<Snowflake> {
    if text.is_empty() || text.len() > 20 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let raw: u64 = text.parse().ok()?;
    (raw != 0).then(|| Snowflake::new(raw))
}

impl fmt::Display for DeepLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = |f: &mut fmt::Formatter<'_>, message_id: &Option<Snowflake>| match message_id
        {
            Some(message_id) => write!(f, "?message={message_id}"),
            None => Ok(()),
        };
        match self {
            Self::Channel {
                guild_id,
                channel_id,
                message_id,
            } => {
                write!(f, "{SCHEME}://channel/{guild_id}/{channel_id}")?;
                message(f, message_id)
            }
            Self::Dm {
                channel_id,
                message_id,
            } => {
                write!(f, "{SCHEME}://dm/{channel_id}")?;
                message(f, message_id)
            }
            Self::FriendRequests => write!(f, "{SCHEME}://friends/requests"),
        }
    }
}

/// Why a link did not parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("not an evre:// link")]
pub struct NotADeepLink;

impl FromStr for DeepLink {
    type Err = NotADeepLink;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text).ok_or(NotADeepLink)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(raw: u64) -> Snowflake {
        Snowflake::new(raw)
    }

    #[test]
    fn links_build_and_parse() {
        let cases = [
            (
                DeepLink::Channel {
                    guild_id: s(1),
                    channel_id: s(2),
                    message_id: Some(s(3)),
                },
                "evre://channel/1/2?message=3",
            ),
            (
                DeepLink::Channel {
                    guild_id: s(1),
                    channel_id: s(2),
                    message_id: None,
                },
                "evre://channel/1/2",
            ),
            (
                DeepLink::Dm {
                    channel_id: s(9),
                    message_id: Some(s(10)),
                },
                "evre://dm/9?message=10",
            ),
            (
                DeepLink::Dm {
                    channel_id: s(9),
                    message_id: None,
                },
                "evre://dm/9",
            ),
            (DeepLink::FriendRequests, "evre://friends/requests"),
        ];
        for (link, text) in cases {
            assert_eq!(link.to_string(), text);
            assert_eq!(DeepLink::parse(text), Some(link));
            assert_eq!(text.parse::<DeepLink>(), Ok(link));
        }
    }

    #[test]
    fn parsing_is_lenient_where_harmless_and_strict_on_ids() {
        let dm = |message| DeepLink::Dm {
            channel_id: s(9),
            message_id: message,
        };
        assert_eq!(DeepLink::parse(" EVRE://dm/9/ "), Some(dm(None)));
        assert_eq!(
            DeepLink::parse("evre://dm/9?x=1&message=5#top"),
            Some(dm(Some(s(5))))
        );
        assert_eq!(
            DeepLink::parse("evre://friends/requests/"),
            Some(DeepLink::FriendRequests)
        );
        for bad in [
            "",
            "evre:dm/9",
            "https://evre.chat/dm/9",
            "evre://dm/",
            "evre://dm/-9",
            "evre://dm/+9",
            "evre://dm/0",
            "evre://dm/9x",
            "evre://dm/99999999999999999999999",
            "evre://dm/9?message=abc",
            "evre://dm/9?message=",
            "evre://channel/1",
            "evre://channel/1/2/3",
            "evre://friends",
            "evre://settings",
        ] {
            assert_eq!(DeepLink::parse(bad), None, "{bad}");
        }
        assert_eq!(
            DeepLink::parse(&format!("evre://dm/9?{}", "a".repeat(600))),
            None
        );
    }
}
