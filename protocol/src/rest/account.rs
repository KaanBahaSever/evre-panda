//! Closing an account and handing over or deleting a guild (ADR-090,
//! `docs/contracts/account-deletion-and-reports.md` §1–§2).

use serde::{Deserialize, Serialize};

use super::Password;
use crate::snowflake::Snowflake;

/// How long a closed account waits before the worker purges it; signing in
/// within it offers to restore the account (`POST /v1/auth/restore`).
pub const ACCOUNT_DELETION_DAYS: u32 = 30;

/// `POST /v1/users/@me/delete` (people only) → 204. Closes the account at
/// once (every session ends, it cannot sign in, nobody finds it) and the
/// worker purges it after [`ACCOUNT_DELETION_DAYS`]. A wrong password is a
/// validation error on `password` and counts like one for a password
/// change; an owner of guilds gets `409`
/// [`ApiError::OwnsGuilds`](crate::ApiError::OwnsGuilds).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAccountRequest {
    /// The account's password.
    pub password: Password,
}

/// How many username changes an account may make in
/// [`USERNAME_CHANGE_WINDOW_DAYS`].
pub const USERNAME_CHANGES_PER_WINDOW: u32 = 2;
/// The window of [`USERNAME_CHANGES_PER_WINDOW`], in days.
pub const USERNAME_CHANGE_WINDOW_DAYS: u32 = 30;

/// `POST /v1/users/@me/username` (people only) → [`User`](crate::User):
/// a new username, with the registration's rules (unique
/// case-insensitively; operator names are reserved). A wrong password is a
/// validation error on `password` (counted like one for a password
/// change), a taken or invalid name one on `username`; more than
/// [`USERNAME_CHANGES_PER_WINDOW`] changes in
/// [`USERNAME_CHANGE_WINDOW_DAYS`] days are `429` with `retryAfterMs`.
/// Every change is kept with its time and address (the operators' records);
/// the old name is free for others at once. Peers and the caller's own
/// sessions get `userUpdate`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeUsernameRequest {
    /// The new username.
    pub username: String,
    /// The account's password.
    pub password: Password,
}

/// `POST /v1/guilds/{guild}/transfer` (the owner only) → 204: `user_id`
/// owns the guild from now on. A wrong password is a validation error on
/// `password`; someone who is not a person member (bots, closed accounts)
/// one on `userId`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferGuildRequest {
    /// The new owner.
    pub user_id: Snowflake,
    /// The current owner's password.
    pub password: Password,
}

/// `POST /v1/guilds/{guild}/delete` (the owner only) → 204: the guild and
/// everything in it are gone for everyone (`guildDelete`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteGuildRequest {
    /// The owner's password.
    pub password: Password,
}
