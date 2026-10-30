//! Gateway (WebSocket) protocol, version [`GATEWAY_VERSION`].
//!
//! Every frame is a [`GatewayMessage`] encoded as `{"op": "<name>", "d": {..}}`
//! (`d` is omitted for opcodes without data).
//!
//! Connection flow (§8.1):
//!
//! 1. server → [`GatewayMessage::Hello`] with the heartbeat interval;
//! 2. client → [`GatewayMessage::Identify`] (new session) or
//!    [`GatewayMessage::Resume`] (continue a dropped session);
//! 3. server → [`GatewayMessage::Ready`] followed by one
//!    [`Event::GuildCreate`] dispatch per guild or, after replaying missed
//!    dispatches, [`GatewayMessage::Resumed`];
//! 4. server → [`GatewayMessage::Dispatch`] events with per-session sequence
//!    numbers starting at 1; client ↔ server heartbeats.
//!
//! Channel-scoped events (messages, typing, channel and voice state changes)
//! reach only sessions whose user has `VIEW_CHANNEL` in that channel. In a
//! DM, messages and typing reach only its participants, as do
//! [`Event::DmCreate`], [`Event::DmUpdate`] and [`Event::DmDelete`].
//! [`Event::InteractionCreate`] reaches only the bot that owns the clicked
//! message, [`Event::EphemeralMessage`] only the person who clicked.
//!
//! The server may send [`GatewayMessage::Reconnect`] (reconnect and resume)
//! or [`GatewayMessage::InvalidSession`] (resume impossible; identify again
//! if `resumable` is `false`).

mod event;

use std::fmt;

use serde::{Deserialize, Serialize};

pub use event::{
    ChannelDelete, DmDelete, Event, GuildDelete, GuildMemberAdd, GuildMemberRemove,
    GuildRoleDelete, JoinRequestCount, JoinRequestUpdate, MessageDelete, Nudge, PresenceUpdate,
    RelationshipRemove, TypingStart,
};

use crate::model::{DeviceForm, DmChannel, Guild, Platform, Relationship, StatusSetting, User};

/// Gateway protocol version, negotiated in the connect URL (`?v=1`).
pub const GATEWAY_VERSION: u32 = 1;

/// WebSocket close codes the server uses. The close reason is an
/// [`ApiError`](crate::ApiError) JSON body when one applies (e.g.
/// `{"code":40100,"message":"unauthorized"}` with
/// [`AUTHENTICATION_FAILED`](close_code::AUTHENTICATION_FAILED)).
///
/// After [`SESSION_TIMED_OUT`](close_code::SESSION_TIMED_OUT), a normal
/// close or a network drop the client resumes; after the others it must not
/// reconnect without fixing the cause (except `UNKNOWN_ERROR` and
/// `RATE_LIMITED`, which allow a delayed reconnect).
pub mod close_code {
    /// Something went wrong on the server; reconnect and resume.
    pub const UNKNOWN_ERROR: u16 = 4000;
    /// The client sent an opcode it must not send.
    pub const UNKNOWN_OPCODE: u16 = 4001;
    /// The client sent a frame that is not a valid [`GatewayMessage`](super::GatewayMessage).
    pub const DECODE_ERROR: u16 = 4002;
    /// The client sent something other than identify or resume first, or
    /// sent nothing in time.
    pub const NOT_AUTHENTICATED: u16 = 4003;
    /// The token in identify or resume is invalid or expired.
    pub const AUTHENTICATION_FAILED: u16 = 4004;
    /// The client identified twice on one connection.
    pub const ALREADY_AUTHENTICATED: u16 = 4005;
    /// The client sends too fast.
    pub const RATE_LIMITED: u16 = 4008;
    /// No heartbeat in time; the session stays resumable.
    pub const SESSION_TIMED_OUT: u16 = 4009;
    /// The `v` query parameter is not [`GATEWAY_VERSION`](super::GATEWAY_VERSION).
    pub const INVALID_VERSION: u16 = 4012;
}

/// One gateway frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "op",
    content = "d",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GatewayMessage {
    /// Server → client, first frame on every connection.
    Hello {
        /// How often the client must send [`GatewayMessage::Heartbeat`].
        heartbeat_interval_ms: u64,
    },
    /// Client → server: authenticate and start a new session.
    Identify {
        /// Access token of the user or bot.
        token: Token,
        /// The device has been idle (no input) for a while already.
        #[serde(default)]
        idle: bool,
        /// What the client runs on; others see the device while the user
        /// is online (§8.4). Older clients omit it (device unknown).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        client: Option<ClientInfo>,
    },
    /// Server → client: a new session is ready.
    Ready {
        /// Opaque session ID, needed to resume. Not a credential.
        session_id: String,
        /// The authenticated user.
        user: User,
        /// Guilds the user is a member of.
        guilds: Vec<Guild>,
        /// The status the user chose (all devices).
        #[serde(default)]
        status: StatusSetting,
        /// The user's DMs, newest activity first. Changes arrive as
        /// [`Event::DmCreate`], [`Event::DmUpdate`] and [`Event::DmDelete`].
        #[serde(default)]
        dms: Vec<DmChannel>,
        /// Friends and pending friend requests (§8.5), friends with their
        /// presence. Changes arrive as [`Event::RelationshipAdd`] and
        /// [`Event::RelationshipRemove`].
        #[serde(default)]
        relationships: Vec<Relationship>,
        /// The users this user blocked (kind
        /// [`RelationshipKind::Blocked`](crate::RelationshipKind::Blocked)),
        /// oldest first. Kept apart from `relationships` so that clients
        /// from before blocking, which fail on a kind they do not know,
        /// still decode `ready`; changes arrive as relationship events like
        /// the others. Empty (and omitted) for bots and from older servers.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        blocked: Vec<Relationship>,
        /// The user's read state in each DM of `dms` (§8.1); guild channels
        /// come with their [`Event::GuildCreate`]. Changes made on other
        /// devices arrive as [`Event::ReadStateUpdate`]. Empty for bots and
        /// from servers without read states.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        read_states: Vec<crate::rest::ReadState>,
    },
    /// Server → client: an event.
    Dispatch {
        /// Per-session sequence number, starting at 1.
        seq: u64,
        /// The event. Unknown event types decode to [`Event::Unknown`].
        #[serde(deserialize_with = "event::deserialize_lenient")]
        event: Event,
    },
    /// Client → server keep-alive. The server may also request one.
    Heartbeat {
        /// Highest `seq` the client has received; 0 if none yet.
        last_seq: u64,
    },
    /// Acknowledges a [`GatewayMessage::Heartbeat`].
    HeartbeatAck,
    /// Client → server: continue a dropped session, replaying missed events
    /// from the server's resume buffer (~5 minutes).
    Resume {
        /// Access token. Required because session IDs appear in logs and must
        /// never authenticate on their own.
        token: Token,
        /// Session to resume, from [`GatewayMessage::Ready`].
        session_id: String,
        /// Highest `seq` the client has received; 0 if none yet.
        last_seq: u64,
    },
    /// Server → client: resume succeeded and missed events were replayed.
    Resumed,
    /// Client → server: this device became idle (no input for a while) or
    /// active again, and/or the user picked a new status. Absent fields stay.
    /// A changed `status` is stored and sent back to all of the user's
    /// sessions as [`Event::StatusSettingUpdate`].
    UpdatePresence {
        /// This device is idle.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        idle: Option<bool>,
        /// The user's chosen status.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<StatusSetting>,
    },
    /// Server → client: reconnect and resume (e.g. server restart).
    Reconnect,
    /// Server → client: the session cannot be continued.
    InvalidSession {
        /// Whether a [`GatewayMessage::Resume`] may still succeed; if
        /// `false`, the client must identify again and resync via REST.
        resumable: bool,
    },
}

/// What a client tells the server about itself in
/// [`GatewayMessage::Identify`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    /// The device's platform, published with the user's presence (§8.4).
    /// Bots' sessions publish none.
    pub os: Platform,
    /// The app version, e.g. `"0.1.19"`. Only logged; never shown to other
    /// users.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The device's form (desktop, laptop, tablet, phone), published with
    /// the platform. Left out when the client cannot tell and by clients
    /// older than forms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<DeviceForm>,
    /// The app's language as an IETF tag (`"de-DE"`), for the texts the
    /// server writes to this person: e-mails and push titles and bodies
    /// (ADR-083). One of [`crate::Locale::ALL`] by its primary subtag, else
    /// English; left out by older clients, which keeps what the account
    /// has (Turkish if it never said). A tag rather than [`crate::Locale`]
    /// so a language this server does not know never fails the identify.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
}

/// A bearer token. Its `Debug` output is redacted so tokens never end up in
/// logs (§11.1).
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Token(String);

impl Token {
    /// Wraps a token string.
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    /// The raw token, for authentication only.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_debug_is_redacted() {
        let message = GatewayMessage::Identify {
            token: Token::new("secret-token"),
            idle: false,
            client: None,
        };
        let debug = format!("{message:?}");
        assert!(!debug.contains("secret-token"), "{debug}");
        assert!(debug.contains("<redacted>"));
        assert_eq!(Token::new("abc").expose(), "abc");
    }
}
