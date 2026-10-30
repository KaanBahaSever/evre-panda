//! Wire-format tests for message components and interactions (§8.1,
//! ADR-059): the JSON shapes, and that older peers on both sides keep
//! working (messages without the new fields, unknown component types,
//! styles, answers and events).

use protocol::model::{
    ActionRow, Button, ButtonStyle, Component, Interaction, InteractionKind, MessageReply,
};
use protocol::rest::{
    CreateInteractionRequest, CreateMessageRequest, InteractionCallback, InteractionFollowup,
    InteractionResponse, InteractionResponseKind, UpdateMessageRequest,
};
use protocol::{ApiError, Event, GatewayMessage, Message, Snowflake, User};
use serde_json::{Value, json};

const CHANNEL: Snowflake = Snowflake::new(300);
const BOT: Snowflake = Snowflake::new(200);

fn skip() -> Button {
    Button {
        style: ButtonStyle::Secondary,
        label: "Geç".into(),
        emoji: Some("⏭".into()),
        custom_id: Some("panda:skip".into()),
        url: None,
        disabled: false,
    }
}

fn panel() -> Message {
    Message {
        id: Snowflake::new(501),
        channel_id: CHANNEL,
        author_id: BOT,
        content: "🐼 Şimdi çalıyor: **Gülpembe**".into(),
        nonce: None,
        edited_at: None,
        attachments: Vec::new(),
        mentions: Vec::new(),
        reply: None,
        components: vec![ActionRow::new([
            skip().into(),
            Button {
                style: ButtonStyle::Danger,
                label: "Dur".into(),
                emoji: None,
                custom_id: Some("panda:stop".into()),
                url: None,
                disabled: true,
            }
            .into(),
        ])],
        ephemeral: false,
        author: None,
    }
}

fn round_trip<T>(value: &T) -> Value
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_value(value).unwrap();
    let back: T = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(&back, value);
    json
}

#[test]
fn component_json_shape() {
    let json = round_trip(&panel());
    assert_eq!(
        json["components"],
        json!([{"components": [
            {"type": "button", "style": "secondary", "label": "Geç", "emoji": "⏭", "customId": "panda:skip"},
            {"type": "button", "style": "danger", "label": "Dur", "customId": "panda:stop", "disabled": true},
        ]}])
    );
    assert!(json.get("ephemeral").is_none(), "false is left out: {json}");

    let link = Button {
        style: ButtonStyle::Link,
        label: "Kaynak".into(),
        emoji: None,
        custom_id: None,
        url: Some("https://evre.chat".into()),
        disabled: false,
    };
    assert_eq!(
        round_trip(&Component::Button(link)),
        json!({"type": "button", "style": "link", "label": "Kaynak", "url": "https://evre.chat"})
    );
}

#[test]
fn messages_without_components_keep_the_old_shape() {
    // What older servers send, and what plain messages look like.
    let old: Message = serde_json::from_value(json!({
        "id": "1", "channelId": "2", "authorId": "3", "content": "selam",
        "nonce": null, "editedAt": null,
    }))
    .unwrap();
    assert!(old.components.is_empty());
    assert!(!old.ephemeral);
    let json = serde_json::to_value(&old).unwrap();
    assert!(json.get("components").is_none(), "{json}");
    assert!(json.get("ephemeral").is_none(), "{json}");
}

#[test]
fn newer_component_types_and_styles_decode_as_unknown() {
    let message: Message = serde_json::from_value(json!({
        "id": "1", "channelId": "2", "authorId": "3", "content": "",
        "nonce": null, "editedAt": null,
        "components": [{"components": [
            {"type": "select", "customId": "x", "options": [{"label": "a"}]},
            {"type": "button", "style": "premium", "label": "Al", "customId": "buy", "sku": "9"},
        ]}],
    }))
    .unwrap();
    let row = &message.components[0].components;
    assert_eq!(row[0], Component::Unknown);
    let button = row[1].as_button().unwrap();
    assert_eq!(button.style, ButtonStyle::Unknown);
    assert_eq!(button.custom_id.as_deref(), Some("buy"));
    // Unknown components still encode (to the UI), as a plain marker.
    assert_eq!(
        serde_json::to_value(&row[0]).unwrap(),
        json!({"type": "unknown"})
    );
}

#[test]
fn message_requests_stay_compatible() {
    let plain = CreateMessageRequest {
        content: "selam".into(),
        nonce: None,
        attachment_ids: Vec::new(),
        reply_to: None,
        mention_replied: false,
        components: Vec::new(),
    };
    assert_eq!(
        serde_json::to_value(&plain).unwrap(),
        json!({"content": "selam", "nonce": null})
    );
    let with = CreateMessageRequest {
        components: panel().components,
        ..plain
    };
    assert_eq!(
        round_trip(&with)["components"][0]["components"][0]["customId"],
        "panda:skip"
    );
    let old: CreateMessageRequest = serde_json::from_value(json!({"content": "x"})).unwrap();
    assert!(old.components.is_empty());

    // An edit keeps the components unless it names them; `[]` removes them.
    assert_eq!(
        serde_json::to_value(UpdateMessageRequest::text("yeni")).unwrap(),
        json!({"content": "yeni"})
    );
    let clear = UpdateMessageRequest {
        content: "bitti".into(),
        components: Some(Vec::new()),
    };
    assert_eq!(
        round_trip(&clear),
        json!({"content": "bitti", "components": []})
    );
    let old: UpdateMessageRequest = serde_json::from_value(json!({"content": "x"})).unwrap();
    assert_eq!(old.components, None);
}

#[test]
fn interaction_bodies() {
    assert_eq!(
        round_trip(&CreateInteractionRequest {
            custom_id: "panda:skip".into()
        }),
        json!({"customId": "panda:skip"})
    );
    assert_eq!(
        round_trip(&InteractionCallback::UpdateMessage {
            content: None,
            components: Some(Vec::new()),
        }),
        json!({"type": "updateMessage", "components": []})
    );
    assert_eq!(
        round_trip(&InteractionCallback::Ephemeral {
            content: "Sıra boş".into()
        }),
        json!({"type": "ephemeral", "content": "Sıra boş"})
    );
    assert_eq!(
        round_trip(&InteractionCallback::Defer),
        json!({"type": "defer"})
    );
    assert_eq!(
        round_trip(&InteractionFollowup {
            content: "tamam".into()
        }),
        json!({"content": "tamam"})
    );
    let deferred = InteractionResponse {
        id: Snowflake::new(9),
        kind: InteractionResponseKind::Deferred,
        message: None,
    };
    assert_eq!(
        round_trip(&deferred),
        json!({"id": "9", "kind": "deferred"})
    );
    // An answer kind from a newer server.
    let newer: InteractionResponse =
        serde_json::from_value(json!({"id": "9", "kind": "modal", "modal": {}})).unwrap();
    assert_eq!(newer.kind, InteractionResponseKind::Unknown);
}

#[test]
fn interaction_events() {
    let interaction = Interaction {
        id: Snowflake::new(900),
        kind: InteractionKind::Button,
        guild_id: Snowflake::new(100),
        channel_id: CHANNEL,
        message: panel(),
        custom_id: "panda:skip".into(),
        user: User {
            id: Snowflake::new(7),
            username: "kaan".into(),
            display_name: None,
            avatar: None,
            banner: None,
            bio: None,
            bot: false,
            deleted: false,
        },
    };
    let frame = GatewayMessage::Dispatch {
        seq: 4,
        event: Event::InteractionCreate(interaction.clone()),
    };
    let json = round_trip(&frame);
    assert_eq!(json["d"]["event"]["t"], "interactionCreate");
    assert_eq!(json["d"]["event"]["d"]["customId"], "panda:skip");
    assert_eq!(json["d"]["event"]["d"]["kind"], "button");
    assert_eq!(json["d"]["event"]["d"]["message"]["id"], "501");
    // A kind from a newer server, or none.
    let mut raw = serde_json::to_value(&interaction).unwrap();
    raw["kind"] = json!("modalSubmit");
    let newer: Interaction = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(newer.kind, InteractionKind::Unknown);
    raw.as_object_mut().unwrap().remove("kind");
    let bare: Interaction = serde_json::from_value(raw).unwrap();
    assert_eq!(bare.kind, InteractionKind::Button);

    let ephemeral = Message {
        id: Snowflake::new(502),
        content: "Sıra boş.".into(),
        components: Vec::new(),
        ephemeral: true,
        reply: Some(MessageReply {
            message_id: Snowflake::new(501),
            author_id: Some(BOT),
            snippet: "🐼 Şimdi çalıyor".into(),
            attachment_count: 0,
            deleted: false,
            mention: false,
            author: None,
        }),
        ..panel()
    };
    let frame = GatewayMessage::Dispatch {
        seq: 5,
        event: Event::EphemeralMessage(ephemeral),
    };
    let json = round_trip(&frame);
    assert_eq!(json["d"]["event"]["t"], "ephemeralMessage");
    assert_eq!(json["d"]["event"]["d"]["ephemeral"], true);
    assert!(json["d"]["event"]["d"].get("components").is_none());
}

#[test]
fn interaction_errors() {
    let offline = ApiError::bot_unavailable();
    assert_eq!(offline.code(), protocol::error::code::BOT_UNAVAILABLE);
    assert_eq!(offline.http_status(), 503);
    let late = ApiError::interaction_timeout();
    assert_eq!(late.code(), 50_400);
    assert_eq!(late.http_status(), 504);
    // Decoded by any client as an unknown code, kept intact.
    let json = serde_json::to_value(&late).unwrap();
    assert_eq!(json["code"], 50_400);
    let back: ApiError = serde_json::from_value(json).unwrap();
    assert_eq!(back.code(), 50_400);
}
