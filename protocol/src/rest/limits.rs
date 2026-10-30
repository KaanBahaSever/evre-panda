//! Input limits enforced by the server. Clients check them before sending so
//! users see problems immediately. Lengths count Unicode scalar values
//! (`str::chars`), not bytes.

use std::ops::RangeInclusive;

/// Username length. Allowed characters: ASCII letters, digits, `_`, `.`, `-`.
pub const USERNAME_LEN: RangeInclusive<usize> = 2..=32;

/// Password length.
pub const PASSWORD_LEN: RangeInclusive<usize> = 8..=128;

/// Display name length.
pub const DISPLAY_NAME_LEN: RangeInclusive<usize> = 1..=32;

/// Guild name length.
pub const GUILD_NAME_LEN: RangeInclusive<usize> = 2..=100;

/// Channel name length.
pub const CHANNEL_NAME_LEN: RangeInclusive<usize> = 1..=100;

/// Maximum channel topic length.
pub const MAX_TOPIC_LEN: usize = 1024;

/// Role name length.
pub const ROLE_NAME_LEN: RangeInclusive<usize> = 1..=100;

/// Message content length.
pub const MESSAGE_LEN: RangeInclusive<usize> = 1..=4000;

/// Characters of the original's text a reply carries at most
/// (`MessageReply::snippet`, the `…` not counted).
pub const REPLY_SNIPPET_LEN: usize = 100;

/// Maximum message nonce length (a ULID has 26 characters).
pub const MAX_NONCE_LEN: usize = 64;

/// Messages returned by a history request when `limit` is not given.
pub const DEFAULT_MESSAGE_PAGE: u32 = 50;

/// Highest accepted `limit` for a history request.
pub const MAX_MESSAGE_PAGE: u32 = 100;

/// Maximum profile bio length (newlines allowed).
pub const MAX_BIO_LEN: usize = 190;

/// Maximum ban reason length (one line, §10).
pub const MAX_BAN_REASON_LEN: usize = 512;

/// Largest avatar or guild icon upload in bytes (8 MiB).
pub const MAX_AVATAR_BYTES: u64 = 8 * 1024 * 1024;

/// Largest user or guild banner upload in bytes (10 MiB).
pub const MAX_BANNER_BYTES: u64 = 10 * 1024 * 1024;

/// Largest width and height of an uploaded profile or guild image, in
/// pixels.
pub const MAX_IMAGE_SIDE: u32 = 4096;

/// Participants of a DM at most, the creator included: a 1:1 DM has two, a
/// group DM up to this many.
pub const MAX_DM_RECIPIENTS: usize = 10;

/// Group DM name length.
pub const DM_NAME_LEN: RangeInclusive<usize> = 1..=100;

/// Unread and mention counts of a read state stop here (§8.1): the server
/// counts at most this many messages per channel, and clients show this
/// value as "99+".
pub const MAX_UNREAD_COUNT: u32 = 100;

/// Rows of components under a message at most (§8.1 components).
pub const MAX_COMPONENT_ROWS: usize = 5;

/// Components in one row at most.
pub const MAX_ROW_COMPONENTS: usize = 5;

/// Characters of a button's label at most.
pub const BUTTON_LABEL_LEN: usize = 80;

/// Characters of a button's emoji at most (one emoji, which can be a
/// sequence of several code points).
pub const BUTTON_EMOJI_LEN: usize = 16;

/// Length of a component's `customId`.
pub const CUSTOM_ID_LEN: RangeInclusive<usize> = 1..=100;

/// Length of a link button's URL at most.
pub const MAX_BUTTON_URL_LEN: usize = 512;

/// How long the server waits for a bot's answer to an interaction (its
/// callback) before the click fails with `50400`, in milliseconds.
pub const INTERACTION_DEADLINE_MS: u64 = 3000;

/// How long after an interaction its bot may still send followups, in
/// seconds.
pub const INTERACTION_FOLLOWUP_SECS: u64 = 900;
