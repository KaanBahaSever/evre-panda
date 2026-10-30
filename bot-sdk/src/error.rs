//! Errors of the SDK.

use protocol::ApiError;

/// What can go wrong talking to Era.
///
/// Error texts never contain the bot token or full URLs.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The base URL is not an `http(s)://host[:port]` address.
    #[error("invalid base URL: {0}")]
    InvalidUrl(String),
    /// The token is not an `erabot_…` bot token.
    #[error("not a bot token (expected erabot_…)")]
    InvalidToken,
    /// The server answered with an error ([`ApiError::Unauthorized`] for a
    /// wrong or reset token, [`ApiError::Forbidden`] for a missing
    /// permission, ...).
    #[error("server error: {0}")]
    Api(#[from] ApiError),
    /// The request did not reach the server or the answer did not arrive.
    #[error("network error: {0}")]
    Network(String),
    /// The request timed out.
    #[error("request timed out")]
    Timeout,
    /// The server sent something this SDK cannot decode.
    #[error("unexpected response: {0}")]
    Decode(String),
    /// The gateway closed the session for good with this close code (see
    /// `protocol::gateway::close_code`).
    #[error("gateway closed the session (code {0})")]
    GatewayClosed(u16),
    /// The first `ready` did not arrive in time.
    #[error("timed out waiting for the gateway to become ready")]
    ReadyTimeout,
    /// The bot was shut down.
    #[error("the bot was shut down")]
    Closed,
    /// Joining, publishing or listening in voice failed.
    #[error("voice: {0}")]
    Voice(String),
}

impl Error {
    /// Whether retrying the same request later may succeed (network
    /// trouble, timeouts, rate limits, server errors).
    #[must_use]
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Network(_) | Self::Timeout => true,
            Self::Api(ApiError::RateLimited { .. } | ApiError::Internal) => true,
            Self::Api(ApiError::Unknown { code, .. }) => *code >= 50_000,
            _ => false,
        }
    }
}

/// Result alias of the SDK.
pub type Result<T, E = Error> = std::result::Result<T, E>;
