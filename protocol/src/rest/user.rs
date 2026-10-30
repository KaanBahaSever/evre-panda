use serde::{Deserialize, Serialize};

use crate::serde_util::double_option;
use crate::timestamp::Timestamp;

/// `PATCH /v1/users/@me`: edits the caller's public profile. Absent fields
/// stay; an explicit `null` clears `displayName` or `bio`. Images are
/// uploaded with `PUT /v1/users/@me/avatar` and `.../banner`; the `clear*`
/// flags remove them. Every change reaches the user's peers as `userUpdate`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMeRequest {
    /// New display name (trimmed, then
    /// [`DISPLAY_NAME_LEN`](super::limits::DISPLAY_NAME_LEN)), or `null` to
    /// show the username.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    pub display_name: Option<Option<String>>,
    /// New bio (trimmed, at most [`MAX_BIO_LEN`](super::limits::MAX_BIO_LEN)
    /// characters, newlines allowed), or `null` (or blank) to clear it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    pub bio: Option<Option<String>>,
    /// Removes the avatar.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clear_avatar: bool,
    /// Removes the banner.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clear_banner: bool,
}

/// The caller's explicit consents (KVKK açık rıza, architecture.md §14,
/// ADR-104), people only:
///
/// - `GET /v1/users/@me/consents` → [`Consents`].
/// - `PATCH /v1/users/@me/consents` with an [`UpdateConsentsRequest`]
///   (absent fields stay) → the new [`Consents`].
///
/// Only the e-mail consent lives here: the push consent belongs to a
/// device's registration (`PUT /v1/users/@me/push`, ADR-087).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Consents {
    /// The consent to transferring the e-mail address to Resend (US), which
    /// sends every mail: without it Evre sends nothing to the address (no
    /// codes, password reset codes or security notices). Given at sign-up
    /// since 2026-10-25; accounts made earlier start without it.
    pub email_transfer: bool,
    /// The [Açık Rıza Metni](super::legal::CONSENT_URL) version it was given
    /// to; absent while it is not given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_transfer_version: Option<String>,
    /// When it was given; absent while it is not given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_transfer_at: Option<Timestamp>,
    /// When it was withdrawn; absent while it is given or was never given.
    /// The apps' one-time prompt after sign-in is only for accounts that
    /// never gave it: a withdrawal is never asked about again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_transfer_withdrawn_at: Option<Timestamp>,
    /// Whether the account has an e-mail address at all (accounts from the
    /// invite-only trial have none): without one the consent sends nothing
    /// either, so the apps do not ask for it.
    #[serde(default)]
    pub has_email: bool,
}

/// `PATCH /v1/users/@me/consents`: gives (`true`) or withdraws (`false`)
/// a consent; absent fields stay. Giving records the current
/// [`CONSENT_VERSION`](super::legal::CONSENT_VERSION) and the time;
/// withdrawing keeps the record that it was given. Both are audited.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConsentsRequest {
    /// The e-mail consent ([`Consents::email_transfer`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_transfer: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consents_are_camel_case_and_unset_fields_stay_out() {
        let given = Consents {
            email_transfer: true,
            email_transfer_version: Some("2026-10-28".into()),
            email_transfer_at: Some(Timestamp::from_unix_millis(1_000)),
            email_transfer_withdrawn_at: None,
            has_email: true,
        };
        assert_eq!(
            serde_json::to_value(&given).unwrap(),
            serde_json::json!({
                "emailTransfer": true,
                "emailTransferVersion": "2026-10-28",
                "emailTransferAt": 1000,
                "hasEmail": true,
            })
        );
        assert_eq!(
            serde_json::to_value(Consents::default()).unwrap(),
            serde_json::json!({"emailTransfer": false, "hasEmail": false})
        );
        let withdrawn = Consents {
            email_transfer_withdrawn_at: Some(Timestamp::from_unix_millis(2_000)),
            ..Consents::default()
        };
        assert_eq!(
            serde_json::to_value(withdrawn).unwrap()["emailTransferWithdrawnAt"],
            2000
        );
        // A newer server's extra fields are ignored.
        let decoded: Consents =
            serde_json::from_value(serde_json::json!({"emailTransfer": false, "push": true}))
                .unwrap();
        assert_eq!(decoded, Consents::default());
        assert_eq!(
            serde_json::to_value(UpdateConsentsRequest::default()).unwrap(),
            serde_json::json!({})
        );
        let request: UpdateConsentsRequest =
            serde_json::from_value(serde_json::json!({"emailTransfer": true})).unwrap();
        assert_eq!(request.email_transfer, Some(true));
    }
}
