//! API errors with stable numeric codes.
//!
//! On the wire (REST error bodies and gateway close reasons) an error is
//! `{"code": 40400, "message": "not found"}` plus variant-specific fields.
//! Codes are `HTTP status × 100 + n`; clients branch on `code`, never on
//! `message`, which is informational English text.

use entitlements::{LimitError, LimitKey};
use serde::{Deserialize, Serialize};

use crate::rest::EmailVerification;

/// Stable numeric error codes. Never change or reuse a value.
pub mod code {
    /// [`ApiError::Validation`](super::ApiError::Validation).
    pub const VALIDATION: u32 = 40_000;
    /// [`ApiError::Unauthorized`](super::ApiError::Unauthorized).
    pub const UNAUTHORIZED: u32 = 40_100;
    /// [`ApiError::Forbidden`](super::ApiError::Forbidden).
    pub const FORBIDDEN: u32 = 40_300;
    /// [`ApiError::PlanLimitReached`](super::ApiError::PlanLimitReached).
    pub const PLAN_LIMIT_REACHED: u32 = 40_301;
    /// [`ApiError::EmailNotVerified`](super::ApiError::EmailNotVerified).
    pub const EMAIL_NOT_VERIFIED: u32 = 40_302;
    /// [`ApiError::AccountPendingDeletion`](super::ApiError::AccountPendingDeletion).
    pub const ACCOUNT_PENDING_DELETION: u32 = 40_303;
    /// [`ApiError::GuildClosed`](super::ApiError::GuildClosed).
    pub const GUILD_CLOSED: u32 = 40_304;
    /// [`ApiError::NotFound`](super::ApiError::NotFound).
    pub const NOT_FOUND: u32 = 40_400;
    /// [`ApiError::OtherDeviceInRoom`](super::ApiError::OtherDeviceInRoom).
    pub const OTHER_DEVICE_IN_ROOM: u32 = 40_921;
    /// [`ApiError::CameraNotPaired`](super::ApiError::CameraNotPaired).
    pub const CAMERA_NOT_PAIRED: u32 = 40_922;
    /// [`ApiError::CameraInUse`](super::ApiError::CameraInUse).
    pub const CAMERA_IN_USE: u32 = 40_923;
    /// [`ApiError::OwnsGuilds`](super::ApiError::OwnsGuilds).
    pub const OWNS_GUILDS: u32 = 40_924;
    /// [`ApiError::JoinRequestRequired`](super::ApiError::JoinRequestRequired).
    pub const JOIN_REQUEST_REQUIRED: u32 = 40_925;
    /// [`ApiError::RateLimited`](super::ApiError::RateLimited).
    pub const RATE_LIMITED: u32 = 42_900;
    /// [`ApiError::Internal`](super::ApiError::Internal).
    pub const INTERNAL: u32 = 50_000;
    /// The bot that owns the message is not connected, so nobody can
    /// answer the interaction (HTTP 503). Sent as [`ApiError::Unknown`](super::ApiError::Unknown)
    /// ([`ApiError::bot_unavailable`](super::ApiError::bot_unavailable)).
    pub const BOT_UNAVAILABLE: u32 = 50_300;
    /// The bot did not answer the interaction in time (HTTP 504). Sent as
    /// [`ApiError::Unknown`](super::ApiError::Unknown)
    /// ([`ApiError::interaction_timeout`](super::ApiError::interaction_timeout)).
    pub const INTERACTION_TIMEOUT: u32 = 50_400;
}

/// An error returned by the REST API or the gateway.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(into = "ErrorBody", from = "ErrorBody")]
pub enum ApiError {
    /// The request is malformed or a field is invalid.
    #[error("invalid request")]
    Validation {
        /// Per-field problems; may be empty.
        errors: Vec<FieldError>,
    },
    /// Missing, expired or invalid credentials.
    #[error("unauthorized")]
    Unauthorized,
    /// Authenticated, but missing a permission.
    #[error("missing permissions")]
    Forbidden,
    /// The guild's plan does not allow this (clients show an upsell).
    #[error("plan limit reached: {key}")]
    PlanLimitReached {
        /// The exceeded entitlement.
        key: LimitKey,
    },
    /// The password was right, but the account's e-mail is not verified
    /// yet: no session until the code from the mail is entered (a login's
    /// answer, §11.7). Older clients see code `40302` as unknown.
    #[error("email not verified")]
    EmailNotVerified {
        /// Where the code went and the ticket to verify it with.
        verification: Box<EmailVerification>,
    },
    /// The password was right, but the account is closed and waits for
    /// its deletion (ADR-090): no session. `POST /v1/auth/restore` with the
    /// same credentials brings it back until `deletion_due_at`.
    #[error("the account is to be deleted")]
    AccountPendingDeletion {
        /// When the worker purges the account.
        deletion_due_at: crate::Timestamp,
    },
    /// The guild takes no new members (`closed`, ADR-095): its invites and
    /// `POST /v1/guilds/{guild}/join` refuse everyone who is not a member.
    /// Clients say "Bu sunucu şu an yeni üye almıyor."
    #[error("the guild takes no new members")]
    GuildClosed,
    /// The resource does not exist or is not visible to the caller.
    #[error("not found")]
    NotFound,
    /// A voice join while the account's connection of the other device
    /// class is in that room (ADR-076): ask the user, then repeat it with
    /// `replaceOtherDevice` (or, on a phone, make it the camera).
    #[error("the account is in this room on another device")]
    OtherDeviceInRoom {
        /// The class of the connection in the room.
        other_device: crate::VoiceDevice,
    },
    /// A camera companion request while the account's desktop voice
    /// connection is not in that channel (ADR-076).
    #[error("no desktop voice connection in this channel")]
    CameraNotPaired,
    /// A camera companion request while the desktop's own camera runs in
    /// that room (ADR-076: one camera per user and room).
    #[error("the desktop camera is on")]
    CameraInUse,
    /// An account deletion while the caller owns guilds (ADR-090): transfer
    /// or delete them first.
    #[error("the account owns guilds")]
    OwnsGuilds {
        /// The guilds the caller owns.
        guild_ids: Vec<crate::Snowflake>,
    },
    /// `POST /v1/guilds/{guild}/join` on a guild in `request` mode
    /// (ADR-095): send a join request instead
    /// (`POST /v1/guilds/{guild}/join-requests`).
    #[error("this guild is joined by request")]
    JoinRequestRequired,
    /// Too many requests.
    #[error("rate limited, retry after {retry_after_ms} ms")]
    RateLimited {
        /// How long to wait before retrying.
        retry_after_ms: u64,
    },
    /// Something went wrong on the server.
    #[error("internal server error")]
    Internal,
    /// A code this build does not know, or a known code with missing fields.
    #[error("{message} (code {code})")]
    Unknown {
        /// The numeric code.
        code: u32,
        /// The server's message.
        message: String,
    },
}

impl ApiError {
    /// A validation error for a single field.
    #[must_use]
    pub fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation {
            errors: vec![FieldError {
                field: field.into(),
                message: message.into(),
            }],
        }
    }

    /// [`code::BOT_UNAVAILABLE`]: the message's bot is offline.
    #[must_use]
    pub fn bot_unavailable() -> Self {
        Self::Unknown {
            code: code::BOT_UNAVAILABLE,
            message: "the bot is not connected".to_owned(),
        }
    }

    /// [`code::INTERACTION_TIMEOUT`]: the bot did not answer in time.
    #[must_use]
    pub fn interaction_timeout() -> Self {
        Self::Unknown {
            code: code::INTERACTION_TIMEOUT,
            message: "the bot did not answer in time".to_owned(),
        }
    }

    /// The stable numeric code (see [`code`]).
    #[must_use]
    pub const fn code(&self) -> u32 {
        match self {
            Self::Validation { .. } => code::VALIDATION,
            Self::Unauthorized => code::UNAUTHORIZED,
            Self::Forbidden => code::FORBIDDEN,
            Self::PlanLimitReached { .. } => code::PLAN_LIMIT_REACHED,
            Self::EmailNotVerified { .. } => code::EMAIL_NOT_VERIFIED,
            Self::AccountPendingDeletion { .. } => code::ACCOUNT_PENDING_DELETION,
            Self::GuildClosed => code::GUILD_CLOSED,
            Self::NotFound => code::NOT_FOUND,
            Self::OtherDeviceInRoom { .. } => code::OTHER_DEVICE_IN_ROOM,
            Self::CameraNotPaired => code::CAMERA_NOT_PAIRED,
            Self::CameraInUse => code::CAMERA_IN_USE,
            Self::OwnsGuilds { .. } => code::OWNS_GUILDS,
            Self::JoinRequestRequired => code::JOIN_REQUEST_REQUIRED,
            Self::RateLimited { .. } => code::RATE_LIMITED,
            Self::Internal => code::INTERNAL,
            Self::Unknown { code, .. } => *code,
        }
    }

    /// The HTTP status code to respond with.
    #[must_use]
    pub const fn http_status(&self) -> u16 {
        match self.code() / 100 {
            status @ 400..=599 => status as u16,
            _ => 500,
        }
    }
}

impl From<LimitError> for ApiError {
    fn from(error: LimitError) -> Self {
        Self::PlanLimitReached { key: error.key }
    }
}

/// A problem with one request field.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldError {
    /// Field path, e.g. `"name"` or `"voice.maxBitrateKbps"`.
    pub field: String,
    /// What is wrong.
    pub message: String,
}

/// Flat wire representation of [`ApiError`].
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorBody {
    code: u32,
    message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    retry_after_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    key: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    errors: Vec<FieldError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    verification: Option<Box<EmailVerification>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    other_device: Option<crate::VoiceDevice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    deletion_due_at: Option<crate::Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    guild_ids: Option<Vec<crate::Snowflake>>,
}

impl From<ApiError> for ErrorBody {
    fn from(error: ApiError) -> Self {
        let mut body = Self {
            code: error.code(),
            message: error.to_string(),
            retry_after_ms: None,
            key: None,
            errors: Vec::new(),
            verification: None,
            other_device: None,
            deletion_due_at: None,
            guild_ids: None,
        };
        match error {
            ApiError::Validation { errors } => body.errors = errors,
            ApiError::PlanLimitReached { key } => body.key = Some(key.as_str().to_owned()),
            ApiError::EmailNotVerified { verification } => body.verification = Some(verification),
            ApiError::RateLimited { retry_after_ms } => body.retry_after_ms = Some(retry_after_ms),
            ApiError::Unknown { message, .. } => body.message = message,
            ApiError::OtherDeviceInRoom { other_device } => body.other_device = Some(other_device),
            ApiError::AccountPendingDeletion { deletion_due_at } => {
                body.deletion_due_at = Some(deletion_due_at);
            }
            ApiError::OwnsGuilds { guild_ids } => body.guild_ids = Some(guild_ids),
            ApiError::Unauthorized
            | ApiError::Forbidden
            | ApiError::NotFound
            | ApiError::CameraNotPaired
            | ApiError::CameraInUse
            | ApiError::GuildClosed
            | ApiError::JoinRequestRequired
            | ApiError::Internal => {}
        }
        body
    }
}

impl From<ErrorBody> for ApiError {
    fn from(body: ErrorBody) -> Self {
        match body.code {
            code::VALIDATION => Self::Validation {
                errors: body.errors,
            },
            code::UNAUTHORIZED => Self::Unauthorized,
            code::FORBIDDEN => Self::Forbidden,
            code::NOT_FOUND => Self::NotFound,
            code::CAMERA_NOT_PAIRED => Self::CameraNotPaired,
            code::CAMERA_IN_USE => Self::CameraInUse,
            code::GUILD_CLOSED => Self::GuildClosed,
            code::JOIN_REQUEST_REQUIRED => Self::JoinRequestRequired,
            code::OTHER_DEVICE_IN_ROOM => match body.other_device {
                Some(other_device) => Self::OtherDeviceInRoom { other_device },
                None => Self::Unknown {
                    code: body.code,
                    message: body.message,
                },
            },
            code::INTERNAL => Self::Internal,
            code::PLAN_LIMIT_REACHED => match body.key.as_deref().map(str::parse) {
                Some(Ok(key)) => Self::PlanLimitReached { key },
                _ => Self::Unknown {
                    code: body.code,
                    message: body.message,
                },
            },
            code::EMAIL_NOT_VERIFIED => match body.verification {
                Some(verification) => Self::EmailNotVerified { verification },
                None => Self::Unknown {
                    code: body.code,
                    message: body.message,
                },
            },
            code::ACCOUNT_PENDING_DELETION => match body.deletion_due_at {
                Some(deletion_due_at) => Self::AccountPendingDeletion { deletion_due_at },
                None => Self::Unknown {
                    code: body.code,
                    message: body.message,
                },
            },
            code::OWNS_GUILDS => match body.guild_ids {
                Some(guild_ids) => Self::OwnsGuilds { guild_ids },
                None => Self::Unknown {
                    code: body.code,
                    message: body.message,
                },
            },
            code::RATE_LIMITED => match body.retry_after_ms {
                Some(retry_after_ms) => Self::RateLimited { retry_after_ms },
                None => Self::Unknown {
                    code: body.code,
                    message: body.message,
                },
            },
            code => Self::Unknown {
                code,
                message: body.message,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use entitlements::LimitValue;

    use super::*;

    #[test]
    fn codes_and_statuses() {
        let cases = [
            (ApiError::validation("name", "too long"), 40_000, 400),
            (ApiError::Unauthorized, 40_100, 401),
            (ApiError::Forbidden, 40_300, 403),
            (
                ApiError::PlanLimitReached {
                    key: LimitKey::MaxMembers,
                },
                40_301,
                403,
            ),
            (ApiError::NotFound, 40_400, 404),
            (ApiError::RateLimited { retry_after_ms: 5 }, 42_900, 429),
            (ApiError::Internal, 50_000, 500),
            (
                ApiError::Unknown {
                    code: 41_800,
                    message: "teapot".into(),
                },
                41_800,
                418,
            ),
            (
                ApiError::Unknown {
                    code: 7,
                    message: "odd".into(),
                },
                7,
                500,
            ),
        ];
        for (error, code, status) in cases {
            assert_eq!(error.code(), code, "{error}");
            assert_eq!(error.http_status(), status, "{error}");
        }
    }

    #[test]
    fn email_not_verified_carries_its_verification() {
        let verification = EmailVerification {
            ticket: Some(crate::Token::new("evv_ticket")),
            email: "k***@g***.com".into(),
            expires_at: crate::Timestamp::from_unix_millis(1_000),
            resend_after_ms: 60_000,
            verified: false,
        };
        let error = ApiError::EmailNotVerified {
            verification: Box::new(verification.clone()),
        };
        assert_eq!(error.code(), 40_302);
        assert_eq!(error.http_status(), 403);
        let json = serde_json::to_value(&error).unwrap();
        assert_eq!(json["code"], 40_302);
        assert_eq!(json["verification"]["email"], "k***@g***.com");
        assert_eq!(json["verification"]["ticket"], "evv_ticket");
        assert_eq!(json["verification"]["resendAfterMs"], 60_000);
        assert_eq!(serde_json::from_value::<ApiError>(json).unwrap(), error);
        // Without its verification the code is unknown, like on older clients.
        let bare: ApiError =
            serde_json::from_value(serde_json::json!({"code": 40_302, "message": "x"})).unwrap();
        assert!(matches!(bare, ApiError::Unknown { code: 40_302, .. }));
    }

    #[test]
    fn other_device_in_room_carries_the_class() {
        let error = ApiError::OtherDeviceInRoom {
            other_device: crate::VoiceDevice::Mobile,
        };
        assert_eq!((error.code(), error.http_status()), (40_921, 409));
        let json = serde_json::to_value(&error).unwrap();
        assert_eq!(json["code"], 40_921);
        assert_eq!(json["otherDevice"], "mobile");
        assert_eq!(serde_json::from_value::<ApiError>(json).unwrap(), error);
        for (error, code) in [
            (ApiError::CameraNotPaired, 40_922),
            (ApiError::CameraInUse, 40_923),
        ] {
            assert_eq!((error.code(), error.http_status()), (code, 409));
            let json = serde_json::to_value(&error).unwrap();
            assert_eq!(serde_json::from_value::<ApiError>(json).unwrap(), error);
        }
    }

    #[test]
    fn account_deletion_errors_carry_their_fields() {
        let pending = ApiError::AccountPendingDeletion {
            deletion_due_at: crate::Timestamp::from_unix_millis(1_790_000_000_000),
        };
        assert_eq!((pending.code(), pending.http_status()), (40_303, 403));
        let json = serde_json::to_value(&pending).unwrap();
        assert_eq!(json["deletionDueAt"], 1_790_000_000_000_u64);
        assert_eq!(serde_json::from_value::<ApiError>(json).unwrap(), pending);
        let owns = ApiError::OwnsGuilds {
            guild_ids: vec![crate::Snowflake::new(7), crate::Snowflake::new(8)],
        };
        assert_eq!((owns.code(), owns.http_status()), (40_924, 409));
        let json = serde_json::to_value(&owns).unwrap();
        assert_eq!(json["guildIds"], serde_json::json!(["7", "8"]));
        assert_eq!(serde_json::from_value::<ApiError>(json).unwrap(), owns);
        // Without their fields the codes are unknown, as on older clients.
        for code in [40_303, 40_924] {
            let bare: ApiError =
                serde_json::from_value(serde_json::json!({"code": code, "message": "x"})).unwrap();
            assert!(matches!(bare, ApiError::Unknown { .. }), "{code}");
        }
    }

    #[test]
    fn guild_access_errors() {
        for (error, code, status) in [
            (ApiError::GuildClosed, 40_304, 403),
            (ApiError::JoinRequestRequired, 40_925, 409),
        ] {
            assert_eq!((error.code(), error.http_status()), (code, status));
            let json = serde_json::to_value(&error).unwrap();
            assert_eq!(json["code"], code);
            assert_eq!(serde_json::from_value::<ApiError>(json).unwrap(), error);
        }
    }

    #[test]
    fn limit_error_converts_to_plan_limit_reached() {
        let error = LimitError {
            key: LimitKey::MaxBots,
            limit: LimitValue::Number(0),
            requested: LimitValue::Number(1),
        };
        assert_eq!(
            ApiError::from(error),
            ApiError::PlanLimitReached {
                key: LimitKey::MaxBots
            }
        );
    }
}
