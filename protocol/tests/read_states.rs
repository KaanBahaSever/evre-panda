//! Read states on the wire (§8.1): the `ReadState` body, its places in
//! `ready` and guild snapshots, and the `readStateUpdate` event.

use protocol::rest::limits::MAX_UNREAD_COUNT;
use protocol::rest::{GuildSnapshot, ReadState};
use protocol::{Event, GatewayMessage, Snowflake, StatusSetting, User};
use serde_json::json;

const CHANNEL: Snowflake = Snowflake::new(300);

fn user() -> User {
    User {
        id: Snowflake::new(100),
        username: "kaan".into(),
        display_name: None,
        avatar: None,
        banner: None,
        bio: None,
        bot: false,
        deleted: false,
    }
}

#[test]
fn read_states_omit_what_is_empty() {
    let unread = ReadState {
        channel_id: CHANNEL,
        last_read_message_id: Some(Snowflake::new(7)),
        last_message_id: Some(Snowflake::new(9)),
        unread_count: 2,
        mention_count: 1,
    };
    let json = serde_json::to_value(unread).unwrap();
    assert_eq!(
        json,
        json!({"channelId": "300", "lastReadMessageId": "7", "lastMessageId": "9", "unreadCount": 2, "mentionCount": 1})
    );
    assert_eq!(serde_json::from_value::<ReadState>(json).unwrap(), unread);
    assert!(unread.is_unread());

    // Never read, nothing there: only the channel.
    let empty = ReadState::empty(CHANNEL);
    let json = serde_json::to_value(empty).unwrap();
    assert_eq!(json, json!({"channelId": "300"}));
    assert_eq!(serde_json::from_value::<ReadState>(json).unwrap(), empty);
    assert!(!empty.is_unread());
    assert!(
        !ReadState {
            last_message_id: Some(Snowflake::new(9)),
            ..empty
        }
        .is_unread()
    );
    assert_eq!(MAX_UNREAD_COUNT, 100, "clients show 100 as 99+");
}

#[test]
fn read_state_updates_round_trip() {
    let event = Event::ReadStateUpdate(ReadState {
        last_read_message_id: Some(Snowflake::new(9)),
        last_message_id: Some(Snowflake::new(9)),
        ..ReadState::empty(CHANNEL)
    });
    assert_eq!(event.event_type(), "readStateUpdate");
    let message = GatewayMessage::Dispatch {
        seq: 4,
        event: event.clone(),
    };
    let json = serde_json::to_value(&message).unwrap();
    assert_eq!(
        json,
        json!({"op": "dispatch", "d": {"seq": 4, "event": {"t": "readStateUpdate", "d": {
            "channelId": "300", "lastReadMessageId": "9", "lastMessageId": "9"
        }}}})
    );
    let decoded: GatewayMessage = serde_json::from_value(json).unwrap();
    assert_eq!(decoded, message);
}

#[test]
fn ready_and_snapshots_carry_read_states_and_decode_without_them() {
    let ready = GatewayMessage::Ready {
        session_id: "s-1".into(),
        user: user(),
        guilds: Vec::new(),
        status: StatusSetting::Online,
        dms: Vec::new(),
        relationships: Vec::new(),
        blocked: Vec::new(),
        read_states: vec![ReadState {
            unread_count: MAX_UNREAD_COUNT,
            last_message_id: Some(Snowflake::new(500)),
            ..ReadState::empty(CHANNEL)
        }],
    };
    let json = serde_json::to_value(&ready).unwrap();
    assert_eq!(json["d"]["readStates"][0]["unreadCount"], 100);
    assert_eq!(
        serde_json::from_value::<GatewayMessage>(json).unwrap(),
        ready
    );

    // An older server sends neither field.
    let old_ready = json!({"op": "ready", "d": {
        "sessionId": "s-1", "user": serde_json::to_value(user()).unwrap(), "guilds": []
    }});
    let GatewayMessage::Ready { read_states, .. } = serde_json::from_value(old_ready).unwrap()
    else {
        panic!("expected ready");
    };
    assert!(read_states.is_empty());

    let snapshot = json!({
        "guild": {"id": "1", "name": "Bizim Sunucu", "ownerId": "100", "plan": "free"},
        "channels": [], "roles": [], "members": [], "voiceStates": [],
        "permissions": "0", "channelPermissions": []
    });
    let decoded: GuildSnapshot = serde_json::from_value(snapshot).unwrap();
    assert!(decoded.read_states.is_empty());
    // Without read states the field is left out (bots, older servers).
    let json = serde_json::to_value(&decoded).unwrap();
    assert!(json.get("readStates").is_none(), "{json}");
}
