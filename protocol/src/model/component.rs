//! Message components: rows of buttons that bots put under their messages
//! (architecture.md §8.1 components and interactions, ADR-059).
//!
//! A click on a button becomes an interaction: the client posts it, the
//! server forwards it to the bot that owns the message (`interactionCreate`),
//! and the bot answers by updating the message, with an ephemeral reply only
//! the clicker sees, or by deferring (`protocol::rest::InteractionCallback`).
//!
//! Limits are in [`limits`](crate::rest::limits) (`MAX_COMPONENT_ROWS`,
//! `MAX_ROW_COMPONENTS`, ...); the server checks them and only bots may send
//! components.

use serde::{Deserialize, Serialize};

/// One row of components under a message, shown left to right.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionRow {
    /// The row's components, at most
    /// [`MAX_ROW_COMPONENTS`](crate::rest::limits::MAX_ROW_COMPONENTS).
    pub components: Vec<Component>,
}

impl ActionRow {
    /// A row of these components.
    #[must_use]
    pub fn new(components: impl IntoIterator<Item = Component>) -> Self {
        Self {
            components: components.into_iter().collect(),
        }
    }
}

/// A component in an [`ActionRow`], tagged by `type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Component {
    /// A button.
    Button(Button),
    /// A component type introduced after this build (a select menu, ...).
    /// Decoded only; clients skip it and the server never stores one
    /// (sending it is a `400`). It encodes as `{"type": "unknown"}`.
    #[serde(other)]
    Unknown,
}

impl Component {
    /// The button, if this is one.
    #[must_use]
    pub const fn as_button(&self) -> Option<&Button> {
        match self {
            Self::Button(button) => Some(button),
            Self::Unknown => None,
        }
    }
}

impl From<Button> for Component {
    fn from(button: Button) -> Self {
        Self::Button(button)
    }
}

/// A clickable button: a label and/or an emoji in one of a few styles.
///
/// Every button except [`ButtonStyle::Link`] has a `customId`, which comes
/// back in the interaction when someone clicks it; a link button has a
/// `url` instead and opens it (no interaction).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Button {
    /// How it looks.
    pub style: ButtonStyle,
    /// The text, at most
    /// [`BUTTON_LABEL_LEN`](crate::rest::limits::BUTTON_LABEL_LEN)
    /// characters. A button needs a label, an emoji or both.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// A Unicode emoji shown before the label (e.g. `"⏭"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
    /// The bot's own ID for this button, unique within the message
    /// ([`CUSTOM_ID_LEN`](crate::rest::limits::CUSTOM_ID_LEN)). Required
    /// except for link buttons, which must not have one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_id: Option<String>,
    /// For link buttons only: the `http(s)` URL to open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Shown greyed out; clicks are refused.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
}

/// What a [`Button`] looks like (clients map these to their theme).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ButtonStyle {
    /// The main action: accent colour.
    Primary,
    /// A neutral action.
    #[default]
    Secondary,
    /// A positive action: green.
    Success,
    /// A destructive action: red.
    Danger,
    /// Opens `url` instead of sending an interaction.
    Link,
    /// A style introduced after this build; clients draw it as
    /// [`Self::Secondary`]. The server refuses it.
    #[serde(other)]
    Unknown,
}
