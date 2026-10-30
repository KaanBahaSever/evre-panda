use serde::{Deserialize, Serialize};

/// `GET /v1/health`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    /// `"ok"` when the server and its database are reachable.
    pub status: String,
    /// Server version.
    pub version: String,
}

/// `GET /v1/updates/{channel}/{target}/{arch}/{current_version}?installId=…`
/// when an update is offered (otherwise `204 No Content`), architecture.md
/// §5.5.
///
/// The shape is `tauri-plugin-updater`'s dynamic-server format, which is why
/// `pub_date` is snake_case unlike the rest of the API. `signature` is the
/// minisign signature of the artifact produced by the release pipeline; the
/// server only relays it and never holds the signing key. Clients verify it
/// before installing and install only at safe points (never during a call).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateInfo {
    /// The offered version (semver).
    pub version: String,
    /// Release notes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Publication date (RFC 3339).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pub_date: Option<String>,
    /// Download URL of the signed installer or bundle.
    pub url: String,
    /// minisign signature of the file at `url`.
    pub signature: String,
    /// Versions below this must update before joining new calls (after a grace
    /// period). Read by our client from the raw JSON; ignored by the plugin.
    #[serde(
        rename = "minSupportedVersion",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub min_supported_version: Option<String>,
}
