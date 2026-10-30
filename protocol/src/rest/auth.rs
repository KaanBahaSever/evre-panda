use std::fmt;

use serde::{Deserialize, Serialize};

use crate::gateway::Token;
use crate::model::User;
use crate::timestamp::Timestamp;

/// A password in a request body. Its `Debug` output is redacted so passwords
/// never end up in logs (§11.1).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Password(String);

impl Password {
    /// Wraps a password.
    #[must_use]
    pub fn new(password: impl Into<String>) -> Self {
        Self(password.into())
    }

    /// The raw password, for hashing and verification only.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Password(<redacted>)")
    }
}

/// `POST /v1/auth/register` → `201` [`EmailVerification`]. With open
/// registration (the default) anyone may register; on an invite-only server
/// `invite_code` must be a trial registration code from the server
/// configuration. A guild invite code works either way and joins that guild
/// once the e-mail is verified.
///
/// The account starts unverified and gets no session: a 6-digit code goes to
/// `email`, and `POST /v1/auth/verify-email` with it signs in (§11.7).
/// Without the accepted [`legal`](super::legal) versions and the consent the
/// request is refused (`400` on `termsVersion`, `privacyVersion` or
/// `abroadTransferConsent`); older clients that do not send them are told to
/// update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    /// Login handle; see [`limits::USERNAME_LEN`](super::limits::USERNAME_LEN).
    /// Unique case-insensitively.
    pub username: String,
    /// Email address: the verification code goes there, and it is used for
    /// account recovery; never shown to other users. Required. Unique
    /// case-insensitively.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Plain-text password (TLS only); stored as an Argon2id hash.
    pub password: Password,
    /// Optional display name.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Registration code or guild invite code; optional with open
    /// registration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invite_code: Option<String>,
    /// The Kullanım Koşulları version the person accepted: must be
    /// [`legal::TERMS_VERSION`](super::legal::TERMS_VERSION).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terms_version: Option<String>,
    /// The Gizlilik Politikası version the person accepted: must be
    /// [`legal::PRIVACY_VERSION`](super::legal::PRIVACY_VERSION).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privacy_version: Option<String>,
    /// Explicit consent (KVKK "açık rıza", [`legal::CONSENT_URL`](super::legal::CONSENT_URL))
    /// to transfer the e-mail address to Resend in the US, which sends the
    /// verification mail. Required: nobody uses Evre without a verified
    /// address. (Push consent is asked in the apps when notifications are
    /// turned on, not here.)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub abroad_transfer_consent: bool,
}

impl RegisterRequest {
    /// A request with the current legal versions and the consent, as every
    /// client sends it after the person ticked both boxes.
    #[must_use]
    pub fn new(username: impl Into<String>, email: impl Into<String>, password: Password) -> Self {
        Self {
            username: username.into(),
            email: Some(email.into()),
            password,
            display_name: None,
            invite_code: None,
            terms_version: Some(super::legal::TERMS_VERSION.to_owned()),
            privacy_version: Some(super::legal::PRIVACY_VERSION.to_owned()),
            abroad_transfer_consent: true,
        }
    }
}

/// `POST /v1/auth/login` → [`AuthResponse`]. Wrong credentials and unknown
/// handles are the same `401`. After 10 wrong passwords for one handle
/// within 15 minutes it answers `429` for 15 minutes, whatever the address
/// and whether the handle exists. With the right password on an account
/// whose e-mail is not verified yet it answers `403`
/// [`ApiError::EmailNotVerified`](crate::ApiError::EmailNotVerified) instead
/// of a session (and sends a new code when the last one is used up or
/// expired).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    /// Login handle or email address (both case-insensitive).
    pub username: String,
    /// Plain-text password (TLS only).
    pub password: Password,
}

/// Returned by register and login.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponse {
    /// Opaque session token for `Authorization: Bearer` and the gateway. The
    /// server stores only its hash; keep it in the OS credential store.
    pub token: Token,
    /// The signed-in user.
    pub user: User,
    /// When the session expires and the user must sign in again.
    pub expires_at: Timestamp,
}

/// An account waiting for its e-mail code: the `201` answer of
/// registration, and inside the `40302` error of a login on such an account
/// (only after the password was right).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailVerification {
    /// Opaque ticket for [`VerifyEmailRequest`] and
    /// [`ResendVerificationRequest`]: proves the password was given. Valid
    /// for an hour, and at least as long as the current code; the server
    /// keeps only its hash, clients keep it in memory only. Absent in a
    /// resend's answer (the ticket stays the same).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticket: Option<Token>,
    /// The address the code went to, masked (`k***@g***.com`).
    pub email: String,
    /// When the current code stops working (15 minutes after it was sent).
    pub expires_at: Timestamp,
    /// Wait this long before asking for another code (0: now).
    pub resend_after_ms: u64,
    /// The service does not verify e-mail right now: the account is ready,
    /// no code was sent, and the client signs in with its password.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub verified: bool,
}

/// `POST /v1/auth/verify-email` → [`AuthResponse`]: the code from the mail
/// verifies the address and signs in. `400` on `code` (`invalid`, or
/// `expired` once it timed out or 5 wrong tries used it up: ask for a new
/// one) or on `ticket` (`expired`: sign in again).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyEmailRequest {
    /// From [`EmailVerification::ticket`].
    pub ticket: Token,
    /// The six digits from the mail.
    pub code: String,
}

/// `POST /v1/auth/verify-email/resend` → [`EmailVerification`]: sends a new
/// code. At most one a minute and five an hour per account and address
/// (`429` with `retryAfterMs`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResendVerificationRequest {
    /// From [`EmailVerification::ticket`].
    pub ticket: Token,
}

/// `POST /v1/auth/password` → [`SessionsRevoked`]: a new password (the
/// registration rules) after the current one. Every other session of the
/// account ends (its devices are signed out); this one stays. A wrong
/// current password is `400` on `currentPassword` (never `401`, which reads
/// as an expired session); 10 of them within 15 minutes lock password
/// changes for 15 minutes (`429`). New-password problems are `400` on
/// `newPassword`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    /// The password the account has now.
    pub current_password: Password,
    /// The new one.
    pub new_password: Password,
}

/// `POST /v1/auth/password-reset` → [`PasswordReset`] ("Şifremi unuttum",
/// §11.9): mails a 6-digit code to the account's address. The answer is the
/// same whether or not `login` names an account, has an address, is a bot
/// or withdrew its e-mail consent: nobody learns which logins exist (only
/// the holder of the address gets a code). The code is mailed at most once
/// a minute and five times an hour per account and address; per login,
/// client address and for the whole service the requests themselves are
/// limited (`429` with `retryAfterMs`, the same for every login). "Kodu
/// tekrar gönder" is this request again: its new ticket replaces the old.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordResetRequest {
    /// Username or e-mail address (both case-insensitive), as at login.
    pub login: String,
}

/// The answer to a [`PasswordResetRequest`]. It never says where the code
/// went, nor whether one went at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordReset {
    /// Opaque ticket for [`ConfirmPasswordResetRequest`]; the server keeps
    /// only its hash, clients keep it in memory only. Never a session token.
    pub ticket: Token,
    /// When the ticket and its code stop working (15 minutes).
    pub expires_at: Timestamp,
    /// Wait this long before asking again ("Kodu tekrar gönder").
    pub resend_after_ms: u64,
}

/// `POST /v1/auth/password-reset/confirm` → [`AuthResponse`]: the code from
/// the mail sets a new password (the registration rules, `400` on
/// `newPassword`), marks the address verified, ends every session of the
/// account (its devices are signed out), clears the login lockout and signs
/// in. An account waiting for its deletion gets the new password but no
/// session: `403` [`ApiError::AccountPendingDeletion`](crate::ApiError::AccountPendingDeletion)
/// as at login (then `POST /v1/auth/restore` with the new password). `400`
/// on `code` (`invalid`, or `expired` once 5 wrong tries used it up: ask
/// again) or on `ticket` (`expired`: it timed out or was used).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmPasswordResetRequest {
    /// From [`PasswordReset::ticket`].
    pub ticket: Token,
    /// The six digits from the mail.
    pub code: String,
    /// The new password.
    pub new_password: Password,
}

/// How many other sessions a password change or `POST
/// /v1/auth/logout-others` ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionsRevoked {
    /// Other sessions that were signed out (the caller's is kept).
    pub revoked: u32,
}
