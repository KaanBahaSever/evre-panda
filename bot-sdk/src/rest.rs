//! REST helpers for `/v1` (the full route table is in [`protocol::rest`]).
//!
//! Every request carries `Authorization: Bot <token>`. Error bodies decode
//! to [`ApiError`](protocol::ApiError); rate-limited requests are retried
//! after the server's `retryAfterMs` (a few times, briefly), and message
//! sends carry a nonce so a retry after a network error never posts twice.

use std::fmt::Write as _;
use std::time::Duration;

use protocol::model::ActionRow;
use protocol::rest::{
    CreateMessageRequest, GuildMember, GuildSnapshot, InteractionCallback, InteractionFollowup,
    InteractionResponse, MessageQuery, UpdateMeRequest, UpdateMessageRequest,
    UpdateVoiceStateRequest, VoiceJoinRequest, VoiceJoinResponse,
};
use protocol::{ApiError, Channel, Guild, Message, Snowflake, User, VoiceState};
use reqwest::header::{AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, HeaderValue};
use reqwest::{Method, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};

/// The prefix of every bot token.
pub const TOKEN_PREFIX: &str = "erabot_";

/// How often a rate-limited request is retried before the error is returned.
const RATE_LIMIT_RETRIES: u32 = 3;
/// Longest server-requested wait the SDK sleeps through on its own.
const MAX_RATE_LIMIT_WAIT: Duration = Duration::from_secs(10);

/// Normalizes a base URL: `https://` is assumed without a scheme; the path,
/// query, fragment and trailing slash are dropped. Only `http` and `https`
/// are accepted, without credentials.
///
/// ```
/// use era_bot_sdk::rest::normalize_base_url;
/// assert_eq!(normalize_base_url("era.example/").unwrap(), "https://era.example");
/// assert_eq!(normalize_base_url("http://127.0.0.1:8080").unwrap(), "http://127.0.0.1:8080");
/// assert!(normalize_base_url("ftp://era.example").is_err());
/// ```
pub fn normalize_base_url(input: &str) -> Result<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(Error::InvalidUrl("empty".into()));
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };
    let mut url =
        url::Url::parse(&with_scheme).map_err(|error| Error::InvalidUrl(error.to_string()))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(Error::InvalidUrl(format!(
            "unsupported scheme {}",
            url.scheme()
        )));
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(Error::InvalidUrl("no host".into()));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(Error::InvalidUrl("credentials in the URL".into()));
    }
    url.set_path("");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

/// The gateway WebSocket URL for a normalized base URL.
///
/// ```
/// use era_bot_sdk::rest::gateway_url;
/// assert_eq!(gateway_url("https://era.example"), "wss://era.example/v1/gateway?v=1");
/// ```
#[must_use]
pub fn gateway_url(base: &str) -> String {
    let ws = if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        base.to_owned()
    };
    format!("{ws}/v1/gateway?v={}", protocol::GATEWAY_VERSION)
}

/// A random message nonce (26 characters, ULID alphabet).
pub(crate) fn nonce() -> String {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    (0..26)
        .map(|_| char::from(ALPHABET[rand::random_range(0..ALPHABET.len())]))
        .collect()
}

fn network_error(error: &reqwest::Error) -> Error {
    if error.is_timeout() {
        Error::Timeout
    } else {
        // Callers pass errors through `without_url`, so paths stay out of
        // logs.
        Error::Network(error.to_string())
    }
}

/// The error for a non-success response.
fn error_from_response(status: StatusCode, body: &[u8]) -> Error {
    match serde_json::from_slice::<ApiError>(body) {
        Ok(error) => Error::Api(error),
        Err(_) => Error::Api(ApiError::Unknown {
            code: u32::from(status.as_u16()) * 100,
            message: status.canonical_reason().unwrap_or("error").to_owned(),
        }),
    }
}

/// An authenticated REST client for one server. Cheap to clone.
#[derive(Clone)]
pub struct Rest {
    http: reqwest::Client,
    base: String,
    authorization: HeaderValue,
}

impl std::fmt::Debug for Rest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rest")
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}

/// A request body.
enum Body<'a> {
    None,
    Json(Vec<u8>),
    Raw(&'a [u8], &'a str),
}

impl Rest {
    /// A client for `base_url` with a bot token (`erabot_…`).
    pub fn new(base_url: &str, token: &str) -> Result<Self> {
        Self::with_timeout(base_url, token, Duration::from_secs(15))
    }

    /// Like [`Rest::new`] with a request timeout.
    pub fn with_timeout(base_url: &str, token: &str, timeout: Duration) -> Result<Self> {
        let token = token.trim();
        if !token.starts_with(TOKEN_PREFIX) || token.len() <= TOKEN_PREFIX.len() {
            return Err(Error::InvalidToken);
        }
        let mut authorization =
            HeaderValue::from_str(&format!("Bot {token}")).map_err(|_| Error::InvalidToken)?;
        authorization.set_sensitive(true);
        install_crypto_provider();
        let http = reqwest::Client::builder()
            .user_agent(concat!("era-bot-sdk/", env!("CARGO_PKG_VERSION")))
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|error| Error::Network(error.to_string()))?;
        Ok(Self {
            http,
            base: normalize_base_url(base_url)?,
            authorization,
        })
    }

    /// The normalized base URL.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base
    }

    async fn send_once(&self, method: Method, path: &str, body: &Body<'_>) -> Result<Vec<u8>> {
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.base))
            .header(AUTHORIZATION, self.authorization.clone());
        match body {
            Body::None => {}
            Body::Json(bytes) => {
                request = request
                    .header(CONTENT_TYPE, "application/json")
                    .body(bytes.clone());
            }
            Body::Raw(bytes, content_type) => {
                request = request
                    .header(CONTENT_TYPE, *content_type)
                    .header(CONTENT_LENGTH, bytes.len())
                    .body(bytes.to_vec());
            }
        }
        let response = request
            .send()
            .await
            .map_err(|error| network_error(&error.without_url()))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|error| network_error(&error.without_url()))?;
        if status.is_success() {
            Ok(bytes.to_vec())
        } else {
            Err(error_from_response(status, &bytes))
        }
    }

    /// One request, retried while rate limited.
    async fn send(&self, method: Method, path: &str, body: Body<'_>) -> Result<Vec<u8>> {
        let mut attempt = 0;
        loop {
            match self.send_once(method.clone(), path, &body).await {
                Err(Error::Api(ApiError::RateLimited { retry_after_ms }))
                    if attempt < RATE_LIMIT_RETRIES
                        && Duration::from_millis(retry_after_ms) <= MAX_RATE_LIMIT_WAIT =>
                {
                    attempt += 1;
                    log::debug!("rate limited, retrying in {retry_after_ms} ms");
                    tokio::time::sleep(Duration::from_millis(retry_after_ms.max(50))).await;
                }
                other => return other,
            }
        }
    }

    async fn call<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&(impl Serialize + ?Sized)>,
    ) -> Result<T> {
        let body = match body {
            Some(body) => Body::Json(
                serde_json::to_vec(body).map_err(|error| Error::Decode(error.to_string()))?,
            ),
            None => Body::None,
        };
        let bytes = self.send(method, path, body).await?;
        serde_json::from_slice(&bytes).map_err(|error| Error::Decode(error.to_string()))
    }

    async fn call_empty(
        &self,
        method: Method,
        path: &str,
        body: Option<&(impl Serialize + ?Sized)>,
    ) -> Result<()> {
        let body = match body {
            Some(body) => Body::Json(
                serde_json::to_vec(body).map_err(|error| Error::Decode(error.to_string()))?,
            ),
            None => Body::None,
        };
        self.send(method, path, body).await.map(drop)
    }

    // --- the bot itself --------------------------------------------------

    /// `GET /v1/users/@me`: the bot's own account (checks the token).
    pub async fn me(&self) -> Result<User> {
        self.call(Method::GET, "/v1/users/@me", None::<&()>).await
    }

    /// `PATCH /v1/users/@me`: change the bot's display name or bio.
    pub async fn update_me(&self, request: &UpdateMeRequest) -> Result<User> {
        self.call(Method::PATCH, "/v1/users/@me", Some(request))
            .await
    }

    /// `PUT /v1/users/@me/avatar`: a new avatar (PNG, JPEG, GIF or WebP; at
    /// most 8 MiB and 4096×4096).
    pub async fn set_avatar(&self, image: &[u8], content_type: &str) -> Result<User> {
        let bytes = self
            .send(
                Method::PUT,
                "/v1/users/@me/avatar",
                Body::Raw(image, content_type),
            )
            .await?;
        serde_json::from_slice(&bytes).map_err(|error| Error::Decode(error.to_string()))
    }

    /// `DELETE /v1/users/@me/avatar`.
    pub async fn remove_avatar(&self) -> Result<User> {
        self.call(Method::DELETE, "/v1/users/@me/avatar", None::<&()>)
            .await
    }

    // --- guilds, channels, members ----------------------------------------

    /// `GET /v1/users/@me/guilds`: the bot's guild (a bot belongs to one).
    pub async fn guilds(&self) -> Result<Vec<Guild>> {
        self.call(Method::GET, "/v1/users/@me/guilds", None::<&()>)
            .await
    }

    /// `GET /v1/guilds/{guild}`: everything about one guild as the bot sees it.
    pub async fn guild(&self, guild_id: Snowflake) -> Result<GuildSnapshot> {
        self.call(Method::GET, &format!("/v1/guilds/{guild_id}"), None::<&()>)
            .await
    }

    /// `GET /v1/guilds/{guild}/channels`: channels the bot can see.
    pub async fn channels(&self, guild_id: Snowflake) -> Result<Vec<Channel>> {
        self.call(
            Method::GET,
            &format!("/v1/guilds/{guild_id}/channels"),
            None::<&()>,
        )
        .await
    }

    /// `GET /v1/channels/{channel}`.
    pub async fn channel(&self, channel_id: Snowflake) -> Result<Channel> {
        self.call(
            Method::GET,
            &format!("/v1/channels/{channel_id}"),
            None::<&()>,
        )
        .await
    }

    /// `GET /v1/guilds/{guild}/members`: every member with its user.
    pub async fn members(&self, guild_id: Snowflake) -> Result<Vec<GuildMember>> {
        self.call(
            Method::GET,
            &format!("/v1/guilds/{guild_id}/members"),
            None::<&()>,
        )
        .await
    }

    /// `GET /v1/guilds/{guild}/voice-states`: who is in which voice channel.
    pub async fn voice_states(&self, guild_id: Snowflake) -> Result<Vec<VoiceState>> {
        self.call(
            Method::GET,
            &format!("/v1/guilds/{guild_id}/voice-states"),
            None::<&()>,
        )
        .await
    }

    // --- messages ------------------------------------------------------------

    /// `GET /v1/channels/{channel}/messages` (oldest first, see
    /// [`MessageQuery`]).
    pub async fn messages(
        &self,
        channel_id: Snowflake,
        query: MessageQuery,
    ) -> Result<Vec<Message>> {
        let mut path = format!("/v1/channels/{channel_id}/messages");
        let mut separator = '?';
        for (name, value) in [
            ("before", query.before.map(|id| id.to_string())),
            ("after", query.after.map(|id| id.to_string())),
            ("limit", query.limit.map(|limit| limit.to_string())),
        ] {
            if let Some(value) = value {
                let _ = write!(path, "{separator}{name}={value}");
                separator = '&';
            }
        }
        self.call(Method::GET, &path, None::<&()>).await
    }

    /// `POST /v1/channels/{channel}/messages` with Markdown text. Retried
    /// once after a network error with the same nonce, so it posts at most
    /// once.
    pub async fn send_message(
        &self,
        channel_id: Snowflake,
        content: impl Into<String>,
    ) -> Result<Message> {
        self.create_message(
            channel_id,
            &CreateMessageRequest {
                content: content.into(),
                nonce: None,
                attachment_ids: Vec::new(),
                reply_to: None,
                mention_replied: false,
                components: Vec::new(),
            },
        )
        .await
    }

    /// `POST /v1/channels/{channel}/messages` with rows of buttons under
    /// the text ([`components`](crate::components)); the text may be empty.
    pub async fn send_components(
        &self,
        channel_id: Snowflake,
        content: impl Into<String>,
        components: Vec<ActionRow>,
    ) -> Result<Message> {
        self.create_message(
            channel_id,
            &CreateMessageRequest {
                content: content.into(),
                nonce: None,
                attachment_ids: Vec::new(),
                reply_to: None,
                mention_replied: false,
                components,
            },
        )
        .await
    }

    /// `POST /v1/channels/{channel}/messages` as a reply to `message_id`, a
    /// message of that channel (§8.1): clients show it with a quote of the
    /// original. With `mention` the original's author is pinged (it counts
    /// as a mention for them). Servers before replies store a plain
    /// message.
    pub async fn send_reply(
        &self,
        channel_id: Snowflake,
        message_id: Snowflake,
        content: impl Into<String>,
        mention: bool,
    ) -> Result<Message> {
        self.create_message(
            channel_id,
            &CreateMessageRequest {
                content: content.into(),
                nonce: None,
                attachment_ids: Vec::new(),
                reply_to: Some(message_id),
                mention_replied: mention,
                components: Vec::new(),
            },
        )
        .await
    }

    /// `POST /v1/channels/{channel}/messages` with a full request. A missing
    /// nonce is generated.
    pub async fn create_message(
        &self,
        channel_id: Snowflake,
        request: &CreateMessageRequest,
    ) -> Result<Message> {
        let mut request = request.clone();
        if request.nonce.is_none() {
            request.nonce = Some(nonce());
        }
        let path = format!("/v1/channels/{channel_id}/messages");
        match self.call(Method::POST, &path, Some(&request)).await {
            Err(Error::Network(_) | Error::Timeout) => {
                self.call(Method::POST, &path, Some(&request)).await
            }
            other => other,
        }
    }

    /// `PATCH /v1/channels/{channel}/messages/{message}` (own messages only).
    pub async fn edit_message(
        &self,
        channel_id: Snowflake,
        message_id: Snowflake,
        content: impl Into<String>,
    ) -> Result<Message> {
        self.call(
            Method::PATCH,
            &format!("/v1/channels/{channel_id}/messages/{message_id}"),
            Some(&UpdateMessageRequest::text(content)),
        )
        .await
    }

    /// `PATCH /v1/channels/{channel}/messages/{message}` with a full
    /// request: new text and, with `components`, new buttons (`[]` removes
    /// them).
    pub async fn edit_message_with(
        &self,
        channel_id: Snowflake,
        message_id: Snowflake,
        request: &UpdateMessageRequest,
    ) -> Result<Message> {
        self.call(
            Method::PATCH,
            &format!("/v1/channels/{channel_id}/messages/{message_id}"),
            Some(request),
        )
        .await
    }

    // --- interactions (§8.1) ---------------------------------------------------

    /// `POST /v1/interactions/{interaction}/callback`: the one answer to an
    /// interaction (see [`Interaction`](crate::Interaction) for the
    /// helpers).
    pub async fn interaction_callback(
        &self,
        interaction_id: Snowflake,
        callback: &InteractionCallback,
    ) -> Result<InteractionResponse> {
        self.call(
            Method::POST,
            &format!("/v1/interactions/{interaction_id}/callback"),
            Some(callback),
        )
        .await
    }

    /// `POST /v1/interactions/{interaction}/followup`: another message only
    /// the clicker sees, after the callback.
    pub async fn interaction_followup(
        &self,
        interaction_id: Snowflake,
        content: String,
    ) -> Result<Message> {
        self.call(
            Method::POST,
            &format!("/v1/interactions/{interaction_id}/followup"),
            Some(&InteractionFollowup { content }),
        )
        .await
    }

    /// `DELETE /v1/channels/{channel}/messages/{message}`.
    pub async fn delete_message(&self, channel_id: Snowflake, message_id: Snowflake) -> Result<()> {
        self.call_empty(
            Method::DELETE,
            &format!("/v1/channels/{channel_id}/messages/{message_id}"),
            None::<&()>,
        )
        .await
    }

    /// `POST /v1/channels/{channel}/typing`: shows "typing…" for a few
    /// seconds.
    pub async fn typing(&self, channel_id: Snowflake) -> Result<()> {
        self.call_empty(
            Method::POST,
            &format!("/v1/channels/{channel_id}/typing"),
            None::<&()>,
        )
        .await
    }

    // --- voice ---------------------------------------------------------------

    /// `POST /v1/channels/{channel}/voice/join`: a LiveKit URL and token for
    /// the channel's room. For bots the grants carry the plan's music
    /// bitrate ([`VoiceGrants::max_music_bitrate_kbps`](protocol::rest::VoiceGrants)).
    /// With the `voice` feature, `voice::VoiceConnection::join` does this and
    /// connects.
    pub async fn join_voice(
        &self,
        channel_id: Snowflake,
        request: &VoiceJoinRequest,
    ) -> Result<VoiceJoinResponse> {
        self.call(
            Method::POST,
            &format!("/v1/channels/{channel_id}/voice/join"),
            Some(request),
        )
        .await
    }

    /// `PATCH /v1/guilds/{guild}/voice-states/@me`: own mute/deafen flags
    /// while in voice.
    pub async fn update_voice_state(
        &self,
        guild_id: Snowflake,
        request: &UpdateVoiceStateRequest,
    ) -> Result<VoiceState> {
        self.call(
            Method::PATCH,
            &format!("/v1/guilds/{guild_id}/voice-states/@me"),
            Some(request),
        )
        .await
    }
}

/// Installs ring as the process-wide rustls provider (no-op when one is
/// installed already). reqwest, tokio-tungstenite and LiveKit all use it.
pub(crate) fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_urls_are_normalized() {
        assert_eq!(
            normalize_base_url(" era.example ").unwrap(),
            "https://era.example"
        );
        assert_eq!(
            normalize_base_url("https://era.example/v1/?x=1#y").unwrap(),
            "https://era.example"
        );
        assert_eq!(
            normalize_base_url("http://127.0.0.1:3000/").unwrap(),
            "http://127.0.0.1:3000"
        );
        for bad in [
            "",
            "ws://era.example",
            "https://user:pw@era.example",
            "https://",
        ] {
            assert!(normalize_base_url(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn gateway_urls() {
        assert_eq!(
            gateway_url("http://127.0.0.1:3000"),
            "ws://127.0.0.1:3000/v1/gateway?v=1"
        );
        assert_eq!(
            gateway_url("https://era.example"),
            "wss://era.example/v1/gateway?v=1"
        );
    }

    #[test]
    fn tokens_must_be_bot_tokens() {
        assert_eq!(
            Rest::new("https://era.example", "abc").unwrap_err(),
            Error::InvalidToken
        );
        assert_eq!(
            Rest::new("https://era.example", "erabot_").unwrap_err(),
            Error::InvalidToken
        );
        let rest = Rest::new("https://era.example/", " erabot_x1 ").unwrap();
        assert_eq!(rest.base_url(), "https://era.example");
        // The token never shows in debug output.
        assert!(!format!("{rest:?}").contains("erabot_x1"));
    }

    #[test]
    fn nonces_are_valid() {
        let a = nonce();
        assert_eq!(a.len(), 26);
        assert!(a.bytes().all(|b| b.is_ascii_alphanumeric()));
        assert_ne!(a, nonce());
    }

    #[test]
    fn error_bodies_decode() {
        assert_eq!(
            error_from_response(
                StatusCode::TOO_MANY_REQUESTS,
                br#"{"code":42900,"message":"slow down","retryAfterMs":250}"#
            ),
            Error::Api(ApiError::RateLimited {
                retry_after_ms: 250
            })
        );
        assert_eq!(
            error_from_response(StatusCode::BAD_GATEWAY, b"<html>"),
            Error::Api(ApiError::Unknown {
                code: 50_200,
                message: "Bad Gateway".into()
            })
        );
        assert!(error_from_response(StatusCode::BAD_GATEWAY, b"").is_transient());
        assert!(!Error::Api(ApiError::Forbidden).is_transient());
    }
}
