//! Buttons under messages (architecture.md §8.1 components, ADR-059).
//!
//! A bot attaches rows of buttons to the messages it sends or edits
//! ([`Bot::send_components`](crate::Bot::send_components),
//! [`Bot::edit_components`](crate::Bot::edit_components)). A click comes
//! back as a [`BotEvent::Interaction`](crate::BotEvent::Interaction) with
//! the button's `custom_id`; answer it with
//! [`Interaction::update_message`](crate::Interaction::update_message),
//! [`Interaction::reply_ephemeral`](crate::Interaction::reply_ephemeral) or
//! [`Interaction::defer`](crate::Interaction::defer).
//!
//! ```
//! use era_bot_sdk::components::{Button, row};
//!
//! let rows = vec![
//!     row([
//!         Button::secondary("panda:pause", "Duraklat").emoji("⏯"),
//!         Button::secondary("panda:skip", "Geç").emoji("⏭"),
//!         Button::danger("panda:stop", "Dur").emoji("⏹"),
//!     ]),
//!     row([Button::link("https://evre.chat", "Evre")]),
//! ];
//! assert_eq!(rows[0].components.len(), 3);
//! ```
//!
//! The server enforces the limits of
//! [`protocol::rest::limits`]: at most
//! [`MAX_COMPONENT_ROWS`](protocol::rest::limits::MAX_COMPONENT_ROWS) rows
//! of [`MAX_ROW_COMPONENTS`](protocol::rest::limits::MAX_ROW_COMPONENTS)
//! buttons, labels up to
//! [`BUTTON_LABEL_LEN`](protocol::rest::limits::BUTTON_LABEL_LEN)
//! characters, `custom_id`s unique in the message; a violation is an
//! [`ApiError::Validation`](protocol::ApiError::Validation) naming the
//! button (`components[0].components[2].label`).

pub use protocol::model::{ActionRow, ButtonStyle, Component};

/// A button, built step by step; it turns into a [`Component`] wherever
/// one is expected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Button(protocol::model::Button);

impl Button {
    /// A button that sends an interaction with `custom_id` when clicked.
    #[must_use]
    pub fn new(style: ButtonStyle, custom_id: impl Into<String>, label: impl Into<String>) -> Self {
        Self(protocol::model::Button {
            style,
            label: label.into(),
            emoji: None,
            custom_id: Some(custom_id.into()),
            url: None,
            disabled: false,
        })
    }

    /// The main action (accent colour).
    #[must_use]
    pub fn primary(custom_id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(ButtonStyle::Primary, custom_id, label)
    }

    /// A neutral action.
    #[must_use]
    pub fn secondary(custom_id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(ButtonStyle::Secondary, custom_id, label)
    }

    /// A positive action (green).
    #[must_use]
    pub fn success(custom_id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(ButtonStyle::Success, custom_id, label)
    }

    /// A destructive action (red).
    #[must_use]
    pub fn danger(custom_id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(ButtonStyle::Danger, custom_id, label)
    }

    /// A button that opens an `http(s)` URL; clicks never reach the bot.
    #[must_use]
    pub fn link(url: impl Into<String>, label: impl Into<String>) -> Self {
        Self(protocol::model::Button {
            style: ButtonStyle::Link,
            label: label.into(),
            emoji: None,
            custom_id: None,
            url: Some(url.into()),
            disabled: false,
        })
    }

    /// A Unicode emoji before the label (a button may have only an emoji:
    /// give an empty label).
    #[must_use]
    pub fn emoji(mut self, emoji: impl Into<String>) -> Self {
        self.0.emoji = Some(emoji.into());
        self
    }

    /// Greys the button out; clicks are refused.
    #[must_use]
    pub const fn disabled(mut self, disabled: bool) -> Self {
        self.0.disabled = disabled;
        self
    }

    /// Changes the style (e.g. primary while a mode is on).
    #[must_use]
    pub const fn style(mut self, style: ButtonStyle) -> Self {
        self.0.style = style;
        self
    }

    /// The wire button.
    #[must_use]
    pub fn build(self) -> protocol::model::Button {
        self.0
    }
}

impl From<Button> for Component {
    fn from(button: Button) -> Self {
        Self::Button(button.0)
    }
}

impl From<Button> for protocol::model::Button {
    fn from(button: Button) -> Self {
        button.0
    }
}

/// A row of components, left to right.
#[must_use]
pub fn row<C: Into<Component>>(components: impl IntoIterator<Item = C>) -> ActionRow {
    ActionRow::new(components.into_iter().map(Into::into))
}

/// The `custom_id` of every clickable button in `rows`, in order (handy to
/// check what a message offers).
#[must_use]
pub fn custom_ids(rows: &[ActionRow]) -> Vec<&str> {
    rows.iter()
        .flat_map(|row| &row.components)
        .filter_map(Component::as_button)
        .filter_map(|button| button.custom_id.as_deref())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_the_wire_shape() {
        let rows = vec![row([
            Button::primary("a", "Bir").emoji("1️⃣"),
            Button::danger("b", "").emoji("⏹").disabled(true),
            Button::link("https://evre.chat", "Evre"),
        ])];
        assert_eq!(
            serde_json::to_value(&rows).unwrap(),
            serde_json::json!([{"components": [
                {"type": "button", "style": "primary", "label": "Bir", "emoji": "1️⃣", "customId": "a"},
                {"type": "button", "style": "danger", "emoji": "⏹", "customId": "b", "disabled": true},
                {"type": "button", "style": "link", "label": "Evre", "url": "https://evre.chat"},
            ]}])
        );
        assert_eq!(custom_ids(&rows), ["a", "b"]);
        let restyled = Button::secondary("c", "Tekrar").style(ButtonStyle::Success).build();
        assert_eq!(restyled.style, ButtonStyle::Success);
    }
}
