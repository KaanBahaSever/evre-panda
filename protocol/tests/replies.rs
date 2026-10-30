//! Wire-format tests for replies (§8.1): `Message.reply`, the reply fields
//! of `CreateMessageRequest`, and the shapes of older peers on both sides.

use protocol::model::MessageReply;
use protocol::rest::CreateMessageRequest;
use protocol::{Event, GatewayMessage, Message, Snowflake};
use serde_json::{Value, json};

const CHANNEL: Snowflake = Snowflake::new(300);
const AUTHOR: Snowflake = Snowflake::new(200);
const ORIGINAL: Snowflake = Snowflake::new(500);

fn reply_message(reply: MessageReply) -> Message {
    Message {
        id: Snowflake::new(501),
        channel_id: CHANNEL,
        author_id: Snowflake::new(201),
        content: "aynen".into(),
        nonce: None,
        edited_at: None,
        attachments: Vec::new(),
        mentions: vec![AUTHOR],
        reply: Some(reply),
        components: Vec::new(),
        ephemeral: false,
        author: None,
    }
}

fn round_trip(message: &Message) -> Value {
    let json = serde_json::to_value(message).unwrap();
    let back: Message = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(&back, message);
    json
}

#[test]
fn reply_json_shape() {
    let message = reply_message(MessageReply {
        message_id: ORIGINAL,
        author_id: Some(AUTHOR),
        snippet: "akşam 9'da CS".into(),
        attachment_count: 2,
        deleted: false,
        mention: true,
        author: None,
    });
    assert_eq!(
        round_trip(&message)["reply"],
        json!({
            "messageId": "500",
            "authorId": "200",
            "snippet": "akşam 9'da CS",
            "attachmentCount": 2,
            "mention": true,
        })
    );
    // A deleted original keeps only its ID (and whether the reply pinged).
    let deleted = reply_message(MessageReply::deleted(ORIGINAL, false));
    assert_eq!(
        round_trip(&deleted)["reply"],
        json!({"messageId": "500", "snippet": "", "deleted": true})
    );
}

#[test]
fn plain_messages_have_no_reply_field() {
    let mut message = reply_message(MessageReply::deleted(ORIGINAL, false));
    message.reply = None;
    assert!(round_trip(&message).get("reply").is_none());
    // Messages of older servers decode without one.
    let old: Message = serde_json::from_value(json!({
        "id": "1", "channelId": "2", "authorId": "3", "content": "selam",
        "nonce": null, "editedAt": null,
    }))
    .unwrap();
    assert_eq!(old.reply, None);
    // ...and inside a dispatch.
    let dispatch: GatewayMessage = serde_json::from_value(json!({
        "op": "dispatch",
        "d": {"seq": 4, "event": {"t": "messageCreate", "d": {
            "id": "1", "channelId": "2", "authorId": "3", "content": "x", "nonce": null, "editedAt": null,
        }}},
    }))
    .unwrap();
    assert!(matches!(
        dispatch,
        GatewayMessage::Dispatch { event: Event::MessageCreate(message), .. } if message.reply.is_none()
    ));
}

#[test]
fn reply_fields_may_be_left_out() {
    // A minimal reply (a newer server may add fields; the defaults hold).
    let reply: MessageReply = serde_json::from_value(json!({"messageId": "500"})).unwrap();
    assert_eq!(reply.author_id, None);
    assert_eq!(reply.snippet, "");
    assert_eq!(reply.attachment_count, 0);
    assert!(!reply.deleted && !reply.mention);
    let future: MessageReply =
        serde_json::from_value(json!({"messageId": "500", "authorId": "200", "pinned": true}))
            .unwrap();
    assert_eq!(future.author_id, Some(AUTHOR));
}

#[test]
fn create_message_request_with_and_without_a_reply() {
    let plain = CreateMessageRequest {
        content: "selam".into(),
        nonce: Some("n".into()),
        attachment_ids: Vec::new(),
        reply_to: None,
        mention_replied: false,
        components: Vec::new(),
    };
    // Older servers see exactly the old body.
    assert_eq!(
        serde_json::to_value(&plain).unwrap(),
        json!({"content": "selam", "nonce": "n"})
    );
    let reply = CreateMessageRequest {
        reply_to: Some(ORIGINAL),
        mention_replied: true,
        ..plain.clone()
    };
    assert_eq!(
        serde_json::to_value(&reply).unwrap(),
        json!({"content": "selam", "nonce": "n", "replyTo": "500", "mentionReplied": true})
    );
    // Older clients send neither field.
    let old: CreateMessageRequest = serde_json::from_value(json!({"content": "selam"})).unwrap();
    assert_eq!(old.reply_to, None);
    assert!(!old.mention_replied);
    let back: CreateMessageRequest =
        serde_json::from_value(serde_json::to_value(&reply).unwrap()).unwrap();
    assert_eq!(back, reply);
}
