use serde::{Deserialize, Serialize};

use crate::PlanId;
use crate::model::{Guild, PresenceStatus, User};
use crate::timestamp::Timestamp;

/// `GET /v1/admin/overview`: totals for the operator panel (§11.4). Only for
/// the operators in `ERA_ADMIN_USERNAMES`; everyone else gets `403`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminOverview {
    /// Registered people (bots excluded).
    pub users: u64,
    /// People online right now.
    pub online_users: u64,
    /// Guilds.
    pub guilds: u64,
    /// Messages, DMs included.
    pub messages: u64,
    /// Uploaded files.
    pub files: u64,
    /// Their total size.
    pub file_bytes: u64,
    /// Size of the database.
    pub database_bytes: u64,
}

/// One account in `GET /v1/admin/users`, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminUser {
    /// The public profile.
    pub user: User,
    /// The private email (accounts from the invite-only trial have none).
    pub email: Option<String>,
    /// When the account was created.
    pub created_at: Timestamp,
    /// Last use of any of its sessions.
    pub last_seen_at: Option<Timestamp>,
    /// Presence right now.
    pub status: PresenceStatus,
    /// Guilds it owns.
    pub guilds_owned: u32,
    /// Guilds it is a member of (owned ones included).
    pub guilds_joined: u32,
    /// Total size of the files it uploaded.
    pub upload_bytes: u64,
    /// Signed-in sessions (devices), most recently used first.
    #[serde(default)]
    pub sessions: Vec<AdminSession>,
    /// The latest audited action (login, voice join, …) and where it came
    /// from.
    #[serde(default)]
    pub last_action: Option<AdminAction>,
}

/// A signed-in session of an account, with the address it signed in from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminSession {
    /// Sign-in time.
    pub created_at: Timestamp,
    /// Last use.
    pub last_used_at: Timestamp,
    /// Client IP at sign-in.
    pub ip: Option<String>,
    /// Client port at sign-in.
    pub port: Option<u16>,
    /// The app's user agent.
    pub user_agent: Option<String>,
}

/// An audited action with its address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminAction {
    /// When.
    pub at: Timestamp,
    /// What (`auth.login`, `voice.join`, …).
    pub action: String,
    /// Client IP.
    pub ip: Option<String>,
    /// Client port.
    pub port: Option<u16>,
}

/// One guild in `GET /v1/admin/guilds`, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminGuild {
    /// The guild, plan included.
    pub guild: Guild,
    /// Its owner's username.
    pub owner_username: String,
    /// Members.
    pub members: u32,
    /// Messages in its channels.
    pub messages: u64,
    /// Total size of its files.
    pub file_bytes: u64,
    /// When it was created.
    pub created_at: Timestamp,
}

/// `PATCH /v1/admin/guilds/{guild}`: changes a guild's plan → [`AdminGuild`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetGuildPlanRequest {
    /// The new plan.
    pub plan: PlanId,
}

/// `GET /v1/auth/availability?username=&email=`: whether a sign-up would
/// collide, checked while the form is filled in. A field that was not asked
/// for, or is not valid, is `None`. Rate-limited like the other auth routes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Availability {
    /// The username is free.
    pub username: Option<bool>,
    /// The email is not registered.
    pub email: Option<bool>,
}

/// Query of `GET /v1/auth/availability`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailabilityQuery {
    /// Username to check.
    #[serde(default)]
    pub username: Option<String>,
    /// Email to check.
    #[serde(default)]
    pub email: Option<String>,
}
