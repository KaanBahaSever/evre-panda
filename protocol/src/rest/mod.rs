//! REST API request and response bodies (`/v1`, architecture.md §9).
//!
//! Every route below is relative to the server's base URL. Requests and
//! responses are JSON with the crate's wire conventions; errors are
//! [`ApiError`](crate::ApiError) bodies. Authenticated routes take
//! `Authorization: Bearer <token>` with the token from [`AuthResponse`], or
//! `Authorization: Bot <token>` with a bot's token from [`BotToken`]. Bots
//! cannot log in, create or join guilds, manage bots, or use DMs and
//! friends.
//!
//! | Route | Body → Response |
//! |---|---|
//! | `GET /v1/health` | → [`Health`] |
//! | `POST /v1/auth/register` | [`RegisterRequest`] → `201` [`EmailVerification`] (no session until the e-mail is verified) |
//! | `POST /v1/auth/verify-email` | [`VerifyEmailRequest`] → [`AuthResponse`] |
//! | `POST /v1/auth/verify-email/resend` | [`ResendVerificationRequest`] → [`EmailVerification`] |
//! | `POST /v1/auth/login` | [`LoginRequest`] → [`AuthResponse`] (`403` [`ApiError::EmailNotVerified`](crate::ApiError::EmailNotVerified) while the e-mail is not verified, `403` [`ApiError::AccountPendingDeletion`](crate::ApiError::AccountPendingDeletion) while the account waits for its deletion) |
//! | `POST /v1/auth/restore` | [`LoginRequest`] → [`AuthResponse`]: cancels a pending account deletion (else exactly a login) |
//! | `POST /v1/auth/logout` | → 204 |
//! | `POST /v1/auth/logout-others` | people only: ends every other session → [`SessionsRevoked`] |
//! | `POST /v1/auth/password` | people only, [`ChangePasswordRequest`] → [`SessionsRevoked`] (every other session ends) |
//! | `POST /v1/auth/password-reset` | [`PasswordResetRequest`] → [`PasswordReset`] (the same answer whether the account exists or not) |
//! | `POST /v1/auth/password-reset/confirm` | [`ConfirmPasswordResetRequest`] → [`AuthResponse`] (every session ends; `403` [`ApiError::AccountPendingDeletion`](crate::ApiError::AccountPendingDeletion) like login) |
//! | `GET /v1/auth/availability?username=&email=` | → [`Availability`] |
//! | `GET /v1/admin/overview` | operators only → [`AdminOverview`] |
//! | `GET /v1/admin/users` | operators only → `[AdminUser]` |
//! | `GET /v1/admin/guilds` | operators only → `[AdminGuild]` |
//! | `PATCH /v1/admin/guilds/{guild}` | operators only, [`SetGuildPlanRequest`] → [`AdminGuild`] |
//! | `GET /v1/users/@me` | → [`User`](crate::User) |
//! | `PATCH /v1/users/@me` | [`UpdateMeRequest`] → [`User`](crate::User) |
//! | `POST /v1/users/@me/username` | [`ChangeUsernameRequest`] (people only) → [`User`](crate::User) (at most [`USERNAME_CHANGES_PER_WINDOW`] per [`USERNAME_CHANGE_WINDOW_DAYS`] days, kept in the account's history) |
//! | `POST /v1/users/@me/delete` | [`DeleteAccountRequest`] (people only) → 204: closed now, purged after [`ACCOUNT_DELETION_DAYS`] (`409` [`ApiError::OwnsGuilds`](crate::ApiError::OwnsGuilds) while owning guilds) |
//! | `PUT /v1/users/@me/avatar` | raw image body (`Content-Type`, `Content-Length`; see [`media`]) → [`User`](crate::User) |
//! | `DELETE /v1/users/@me/avatar` | → [`User`](crate::User) |
//! | `PUT /v1/users/@me/banner` | raw image body → [`User`](crate::User) |
//! | `DELETE /v1/users/@me/banner` | → [`User`](crate::User) |
//! | `GET /v1/users/@me/guilds` | → `[Guild]` |
//! | `DELETE /v1/users/@me/guilds/{guild}` | leave → 204 |
//! | `GET /v1/users/@me/dms` | → `[DmChannel]` (newest activity first) |
//! | `POST /v1/users/@me/dms` | [`CreateDmRequest`] → [`DmChannel`](crate::DmChannel) (`201` new, `200` an existing 1:1 DM) |
//! | `DELETE /v1/users/@me/dms/{channel}` | leave a group DM → 204 (`400` for a 1:1 DM) |
//! | `GET /v1/users/@me/relationships` | → `[Relationship]` (friends, pending requests and the users we blocked) |
//! | `POST /v1/users/@me/relationships` | [`FriendRequest`] → [`Relationship`](crate::Relationship) (`404` unknown user, or one who blocked us) |
//! | `PUT /v1/users/@me/relationships/{user}` | send a request to, or accept one from, `user` → [`Relationship`](crate::Relationship) (`404` as above; `400` if we blocked them) |
//! | `DELETE /v1/users/@me/relationships/{user}` | unfriend, cancel or decline → 204 (never unblocks) |
//! | `PUT /v1/users/@me/blocks/{user}` | block `user`: ends a friendship or pending request → [`Relationship`](crate::Relationship) (kind `blocked`) |
//! | `DELETE /v1/users/@me/blocks/{user}` | unblock → 204 (`404` if not blocked) |
//! | `POST /v1/guilds` | [`CreateGuildRequest`] → [`GuildSnapshot`] |
//! | `GET /v1/guilds/{guild}` | → [`GuildSnapshot`] |
//! | `PATCH /v1/guilds/{guild}` | [`UpdateGuildRequest`] → [`Guild`](crate::Guild) (`access` and the Keşfet fields need `MANAGE_GUILD` like the rest) |
//! | `GET /v1/discovery/guilds` | query [`DiscoveryQuery`] → [`DiscoveryPage`] (listed guilds that do not ban the caller; [`DISCOVERY_PER_MINUTE`]) |
//! | `GET /v1/guilds/{guild}/preview` | → [`DiscoveryGuild`] (`public` and `request` guilds, any guild of the caller; `404` otherwise) |
//! | `POST /v1/guilds/{guild}/join` | → [`GuildSnapshot`] (`public`; `409` [`ApiError::JoinRequestRequired`](crate::ApiError::JoinRequestRequired) for `request`, `403` [`ApiError::GuildClosed`](crate::ApiError::GuildClosed) for `closed`, `403` otherwise) |
//! | `POST /v1/guilds/{guild}/join-requests` | [`CreateJoinRequest`] (people only, `request` guilds) → [`JoinRequest`] |
//! | `DELETE /v1/guilds/{guild}/join-requests/@me` | withdraw the pending request → 204 |
//! | `GET /v1/guilds/{guild}/join-requests` | query [`JoinRequestsQuery`] (`KICK_MEMBERS`) → `[GuildJoinRequest]` (pending, oldest first) |
//! | `POST /v1/guilds/{guild}/join-requests/{user}/approve` | (`KICK_MEMBERS`) → 204; the user joins |
//! | `POST /v1/guilds/{guild}/join-requests/{user}/reject` | optional [`RejectJoinRequest`] (`KICK_MEMBERS`) → 204; [`JOIN_REQUEST_COOLDOWN_HOURS`] before they may ask again |
//! | `POST /v1/guilds/{guild}/transfer` | [`TransferGuildRequest`] (the owner) → 204 |
//! | `POST /v1/guilds/{guild}/delete` | [`DeleteGuildRequest`] (the owner) → 204; members get `guildDelete` |
//! | `GET /v1/guilds/{guild}/reports` | query [`GuildReportsQuery`] (`MANAGE_MESSAGES`) → `[GuildReport]` (newest first, the channels where the caller has it) |
//! | `PATCH /v1/guilds/{guild}/reports/{report}` | [`UpdateReportRequest`] (`MANAGE_MESSAGES` in the report's channel) → 204 |
//! | `POST /v1/reports` | [`CreateReportRequest`] (people only) → [`ReportCreated`] |
//! | `PUT /v1/guilds/{guild}/icon` | raw image body (`MANAGE_GUILD`) → [`Guild`](crate::Guild) |
//! | `DELETE /v1/guilds/{guild}/icon` | (`MANAGE_GUILD`) → [`Guild`](crate::Guild) |
//! | `PUT /v1/guilds/{guild}/banner` | raw image body (`MANAGE_GUILD`) → [`Guild`](crate::Guild) |
//! | `DELETE /v1/guilds/{guild}/banner` | (`MANAGE_GUILD`) → [`Guild`](crate::Guild) |
//! | `GET /v1/guilds/{guild}/channels` | → `[Channel]` (visible ones) |
//! | `POST /v1/guilds/{guild}/channels` | [`CreateChannelRequest`] → [`Channel`](crate::Channel) |
//! | `PATCH /v1/guilds/{guild}/channels` | a list of [`ChannelPositionUpdate`] (reorder, move between categories, `lockPermissions` syncs a moved channel with its new category; one transaction) → 204 |
//! | `GET /v1/guilds/{guild}/roles` | → `[Role]` |
//! | `POST /v1/guilds/{guild}/roles` | [`CreateRoleRequest`] → [`Role`](crate::Role) |
//! | `PATCH /v1/guilds/{guild}/roles/{role}` | [`UpdateRoleRequest`] → [`Role`](crate::Role) |
//! | `DELETE /v1/guilds/{guild}/roles/{role}` | → 204 |
//! | `GET /v1/guilds/{guild}/members` | → `[GuildMember]` |
//! | `DELETE /v1/guilds/{guild}/members/{user}` | kick → 204 (`400` for a bot: delete it instead) |
//! | `POST /v1/guilds/{guild}/members/{user}/nudge` | guild owner only: wake a member up (`nudge` event to their sessions) → 204 |
//! | `GET /v1/guilds/{guild}/bans` | (`BAN_MEMBERS`) → `[GuildBan]` (newest first) |
//! | `PUT /v1/guilds/{guild}/bans/{user}` | optional [`CreateBanRequest`] (`BAN_MEMBERS`, higher rank) → 204; removes the member and keeps them out |
//! | `DELETE /v1/guilds/{guild}/bans/{user}` | (`BAN_MEMBERS`) unban → 204 (`404` if not banned) |
//! | `GET /v1/guilds/{guild}/bots` | (`MANAGE_GUILD`) → [`GuildBots`] |
//! | `POST /v1/guilds/{guild}/bots` | [`CreateBotRequest`] (`MANAGE_GUILD`, plan's `max_bots`) → `201` [`BotToken`] |
//! | `POST /v1/guilds/{guild}/bots/{bot}/token` | (`MANAGE_GUILD`) new token, the old one stops working → [`BotToken`] |
//! | `DELETE /v1/guilds/{guild}/bots/{bot}` | (`MANAGE_GUILD`) → 204 |
//! | `PUT /v1/guilds/{guild}/members/{user}/roles/{role}` | → [`Member`](crate::Member) |
//! | `DELETE /v1/guilds/{guild}/members/{user}/roles/{role}` | → [`Member`](crate::Member) |
//! | `GET /v1/guilds/{guild}/invites` | → `[Invite]` |
//! | `POST /v1/guilds/{guild}/invites` | [`CreateInviteRequest`] → [`Invite`](crate::Invite) |
//! | `GET /v1/guilds/{guild}/voice-states` | → `[VoiceState]` (visible channels) |
//! | `PATCH /v1/guilds/{guild}/voice-states/@me` | [`UpdateVoiceStateRequest`] → [`VoiceState`](crate::VoiceState) |
//! | `PATCH /v1/guilds/{guild}/voice-states/{user}` | [`ModerateVoiceStateRequest`] (server mute/deafen) → 204 |
//! | `DELETE /v1/guilds/{guild}/voice-states/{user}` | disconnect from voice (`MOVE_MEMBERS`) → 204 |
//! | `POST /v1/guilds/{guild}/voice-states/{user}/move` | [`MoveVoiceMemberRequest`] → 204 |
//! | `GET /v1/channels/{channel}` | → [`Channel`](crate::Channel) |
//! | `PATCH /v1/channels/{channel}` | [`UpdateChannelRequest`] → [`Channel`](crate::Channel) |
//! | `DELETE /v1/channels/{channel}` | → 204 |
//! | `GET /v1/channels/{channel}/messages` | query [`MessageQuery`] → `[Message]` |
//! | `POST /v1/channels/{channel}/messages` | [`CreateMessageRequest`] → [`Message`](crate::Message) |
//! | `PATCH /v1/channels/{channel}/messages/{message}` | [`UpdateMessageRequest`] → [`Message`](crate::Message) |
//! | `DELETE /v1/channels/{channel}/messages/{message}` | → 204 |
//! | `POST /v1/channels/{channel}/messages/{message}/ack` | mark read up to `message` (people only; never moves back) → [`ReadState`]; the user's other devices get `readStateUpdate` |
//! | `POST /v1/channels/{channel}/messages/{message}/interactions` | [`CreateInteractionRequest`] (people only: a button was clicked) → [`InteractionResponse`] once the message's bot answered |
//! | `POST /v1/interactions/{interaction}/callback` | [`InteractionCallback`] (the interaction's bot) → [`InteractionResponse`] |
//! | `POST /v1/interactions/{interaction}/followup` | [`InteractionFollowup`] (the interaction's bot, after its callback) → [`Message`](crate::Message) (ephemeral) |
//! | `POST /v1/channels/{channel}/typing` | → 204 |
//! | `POST /v1/channels/{channel}/attachments` | raw file body (`Content-Type`, `Content-Length`, `X-Filename` percent-encoded) → [`Attachment`](crate::Attachment) (pending until a message links it) |
//! | `GET /v1/attachments/{attachment}/{filename}` | query `exp`, `sig` (from `Attachment.url`; no auth header) → the file |
//! | `GET /v1/media/{kind}/{owner}/{hash}` | ([`media_path`]; no auth, cached forever) → the image |
//! | `POST /v1/channels/{channel}/voice/join` | [`VoiceJoinRequest`] → [`VoiceJoinResponse`] (`409`/`40921`: the account's other device class is in that room) |
//! | `POST /v1/channels/{channel}/voice/camera` | → [`CameraJoinResponse`] (mobile sessions; `409`/`40922`, `40923`) |
//! | `GET /v1/invites/{code}` | → [`InvitePreview`] (no auth; `access` says whether a `closed` guild refuses it) |
//! | `POST /v1/invites/{code}` | join → [`GuildSnapshot`] (`403` [`ApiError::GuildClosed`](crate::ApiError::GuildClosed) for a `closed` guild) |
//! | `DELETE /v1/invites/{code}` | → 204 |
//! | `PUT /v1/users/@me/push` | [`RegisterPushRequest`] (people only) → [`PushRegistration`] (this login session's push token, §8.1) |
//! | `DELETE /v1/users/@me/push` | → 204 (also ends with sign-out) |
//! | `GET /v1/users/@me/notification-settings` | people only → [`NotificationSettings`] |
//! | `PATCH /v1/users/@me/notification-settings` | [`NotificationSettingsPatch`] (people only; merges, `null` deletes) → [`NotificationSettings`]; every session of the user gets `notificationSettingsUpdate` |
//! | `GET /v1/users/@me/consents` | people only → [`Consents`] (the e-mail consent, ADR-104) |
//! | `PATCH /v1/users/@me/consents` | [`UpdateConsentsRequest`] (people only; absent fields stay) → [`Consents`]: gives or withdraws the e-mail consent |
//! | `GET /v1/updates/{channel}/{target}/{arch}/{version}` | query `installId` → [`UpdateInfo`] or 204 |
//! | `GET /v1/gateway?v=1` | WebSocket upgrade, see [`gateway`](crate::gateway) |
//!
//! The `/v1/channels/{channel}/messages` (and `/messages/{message}`,
//! `/messages/{message}/ack`), `/typing` and `/attachments` routes also take
//! a DM's ID: every
//! participant may read, send, type and upload; edits and deletes are the
//! author's only. For anyone else the DM does not exist (`404`). The other
//! `/v1/channels/{channel}` routes are for guild channels only.
//!
//! Server-internal routes (not for clients): `POST /v1/livekit/webhook`.

mod account;
mod admin;
mod auth;
mod bot;
mod channel;
mod discovery;
mod dm;
mod guild;
mod interaction;
mod invite;
pub mod legal;
pub mod limits;
pub mod media;
mod message;
pub mod notifications;
pub mod push;
mod read_state;
mod relationship;
mod report;
mod update;
mod user;
mod voice;

pub use account::{
    ACCOUNT_DELETION_DAYS, ChangeUsernameRequest, DeleteAccountRequest, DeleteGuildRequest,
    TransferGuildRequest, USERNAME_CHANGE_WINDOW_DAYS, USERNAME_CHANGES_PER_WINDOW,
};
pub use admin::{
    AdminAction, AdminGuild, AdminOverview, AdminSession, AdminUser, Availability,
    AvailabilityQuery, SetGuildPlanRequest,
};
pub use auth::{
    AuthResponse, ChangePasswordRequest, ConfirmPasswordResetRequest, EmailVerification,
    LoginRequest, Password, PasswordReset, PasswordResetRequest, RegisterRequest,
    ResendVerificationRequest, SessionsRevoked, VerifyEmailRequest,
};
pub use bot::{Bot, BotToken, CreateBotRequest, GuildBots};
pub use channel::{ChannelPositionUpdate, CreateChannelRequest, UpdateChannelRequest};
pub use discovery::{
    CreateJoinRequest, DISCOVERY_PER_MINUTE, DiscoveryGuild, DiscoveryPage, DiscoveryQuery,
    DiscoverySort, GuildJoinRequest, JOIN_REQUEST_COOLDOWN_HOURS, JOIN_REQUEST_RETENTION_DAYS,
    JOIN_REQUESTS_PER_HOUR, JoinRequest, JoinRequestStatus, JoinRequestsQuery, MAX_DISCOVERY_PAGE,
    MAX_GUILD_DESCRIPTION_LEN, MAX_JOIN_REQUEST_MESSAGE_LEN, MAX_JOIN_REQUEST_PAGE,
    RejectJoinRequest,
};
pub use dm::CreateDmRequest;
pub use guild::{
    ChannelPermissions, CreateBanRequest, CreateGuildRequest, CreateRoleRequest, GuildBan,
    GuildMember, GuildSnapshot, UpdateGuildRequest, UpdateRoleRequest,
};
pub use interaction::{
    CreateInteractionRequest, InteractionCallback, InteractionFollowup, InteractionResponse,
    InteractionResponseKind,
};
pub use invite::{CreateInviteRequest, InvitePreview};
pub use media::{MediaKind, media_path};
pub use message::{
    CreateMessageRequest, MAX_MESSAGE_ATTACHMENTS, MessageQuery, UpdateMessageRequest,
};
pub use notifications::{
    DmNotificationPatch, DmNotificationSettings, GlobalNotificationPatch,
    GlobalNotificationSettings, GuildNotificationPatch, GuildNotificationSettings, MutePreset,
    MuteUntil, NotificationKind, NotificationLevel, NotificationScope, NotificationSettings,
    NotificationSettingsPatch, NotificationSurface, NotificationTypes, NotificationTypesPatch,
    ScopeNotificationPatch, ScopeNotificationSettings,
};
pub use push::{
    PushData, PushKind, PushProvider, PushRegistration, PushSettings, RegisterPushRequest,
};
pub use read_state::ReadState;
pub use relationship::FriendRequest;
pub use report::{
    CreateReportRequest, GuildReport, GuildReportsQuery, MAX_REPORT_DETAILS_LEN, MAX_REPORT_PAGE,
    REPORTS_PER_HOUR, ReportCreated, ReportReason, ReportSnapshot, ReportStatus, ReportTarget,
    ReportedAttachment, UpdateReportRequest,
};
pub use update::{Health, UpdateInfo};
pub use user::{Consents, UpdateConsentsRequest, UpdateMeRequest};
pub use voice::{
    CameraJoinResponse, DisconnectVoiceMemberQuery, IceServer, ModerateVoiceStateRequest,
    MoveVoiceMemberRequest, UpdateVoiceStateRequest, VoiceGrants, VoiceJoinRequest,
    VoiceJoinResponse, VoiceSource,
};
