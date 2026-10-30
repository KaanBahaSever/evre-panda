use serde::{Deserialize, Serialize};

use crate::snowflake::Snowflake;

/// `POST /v1/users/@me/dms`: opens a 1:1 DM (one recipient) or creates a
/// group DM (two or more, up to
/// [`MAX_DM_RECIPIENTS`](super::limits::MAX_DM_RECIPIENTS) people with the
/// caller).
///
/// Every recipient must be a person (not a bot) who shares a guild with the
/// caller or is their friend, and neither may have blocked the other;
/// otherwise `403`, the same answer for every reason. The same rule holds
/// for every message, edit, upload and typing notice in a 1:1 DM, while its
/// history stays readable.
/// A 1:1 DM between two users is unique: opening it again returns the
/// existing one with `200` (`201` when it is new). Each request with several
/// recipients creates a new group DM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDmRequest {
    /// The other participants (duplicates are ignored; never the caller).
    pub recipient_ids: Vec<Snowflake>,
    /// Name of a group DM
    /// ([`DM_NAME_LEN`](super::limits::DM_NAME_LEN)); not allowed for a 1:1
    /// DM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
