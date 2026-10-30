//! Blocking on the wire (§8.2): the `blocked` relationship kind, kinds from
//! newer servers, `ready.blocked` and what clients from before blocking
//! still decode.

use protocol::{
    Event, GatewayMessage, Relationship, RelationshipKind, Snowflake, StatusSetting, Timestamp,
    User,
};
use serde::Deserialize;
use serde_json::json;

fn user(id: u64, username: &str) -> User {
    User {
        id: Snowflake::new(id),
        username: username.into(),
        display_name: None,
        avatar: None,
        banner: None,
        bio: None,
        bot: false,
        deleted: false,
    }
}

fn blocked(id: u64) -> Relationship {
    Relationship {
        user: user(id, "troll"),
        kind: RelationshipKind::Blocked,
        since: Timestamp::from_unix_millis(1_761_300_000_000),
        presence: None,
    }
}

#[test]
fn blocked_is_its_own_kind_without_presence() {
    let json = serde_json::to_value(blocked(200)).unwrap();
    assert_eq!(json["kind"], "blocked");
    assert_eq!(json["user"]["id"], "200");
    assert!(json.get("presence").is_none(), "blocks share no presence");
    assert_eq!(
        serde_json::from_value::<Relationship>(json).unwrap(),
        blocked(200)
    );

    assert!(RelationshipKind::Friend.is_mutual());
    assert!(RelationshipKind::Incoming.is_mutual());
    assert!(RelationshipKind::Outgoing.is_mutual());
    assert!(!RelationshipKind::Blocked.is_mutual());
    assert!(!RelationshipKind::Unknown.is_mutual());
}

#[test]
fn kinds_from_newer_servers_decode_as_unknown() {
    let mut json = serde_json::to_value(blocked(200)).unwrap();
    json["kind"] = json!("muted");
    let decoded: Relationship = serde_json::from_value(json).unwrap();
    assert_eq!(decoded.kind, RelationshipKind::Unknown);

    // Also inside an event, which therefore still reaches the client.
    let event = json!({"t": "relationshipAdd", "d": {
        "user": serde_json::to_value(user(201, "x")).unwrap(),
        "kind": "muted",
        "since": serde_json::to_value(Timestamp::from_unix_millis(1)).unwrap(),
    }});
    let GatewayMessage::Dispatch { event, .. } =
        serde_json::from_value(json!({"op": "dispatch", "d": {"seq": 4, "event": event}})).unwrap()
    else {
        panic!("expected a dispatch");
    };
    let Event::RelationshipAdd(added) = event else {
        panic!("expected relationshipAdd, got {event:?}");
    };
    assert_eq!(added.kind, RelationshipKind::Unknown);
    assert_eq!(added.user.id, Snowflake::new(201));
}

#[test]
fn relationship_events_carry_blocks() {
    let event = Event::RelationshipAdd(blocked(200));
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["t"], "relationshipAdd");
    assert_eq!(json["d"]["kind"], "blocked");
    assert_eq!(serde_json::from_value::<Event>(json).unwrap(), event);
}

#[test]
fn ready_keeps_blocks_apart_for_older_clients() {
    let friend = Relationship {
        user: user(300, "dost"),
        kind: RelationshipKind::Friend,
        since: Timestamp::from_unix_millis(1_761_200_000_000),
        presence: None,
    };
    let ready = GatewayMessage::Ready {
        session_id: "s-1".into(),
        user: user(100, "kaan"),
        guilds: Vec::new(),
        status: StatusSetting::Online,
        dms: Vec::new(),
        relationships: vec![friend.clone()],
        blocked: vec![blocked(200)],
        read_states: Vec::new(),
    };
    let json = serde_json::to_value(&ready).unwrap();
    assert_eq!(json["d"]["relationships"].as_array().unwrap().len(), 1);
    assert_eq!(json["d"]["blocked"][0]["kind"], "blocked");
    assert_eq!(
        serde_json::from_value::<GatewayMessage>(json.clone()).unwrap(),
        ready
    );

    // Nothing blocked: the field is left out, and older servers never send
    // it.
    let GatewayMessage::Ready {
        session_id,
        user: me,
        guilds,
        status,
        dms,
        relationships,
        read_states,
        ..
    } = ready
    else {
        unreachable!()
    };
    let unblocked = GatewayMessage::Ready {
        session_id,
        user: me,
        guilds,
        status,
        dms,
        relationships,
        blocked: Vec::new(),
        read_states,
    };
    let quiet = serde_json::to_value(&unblocked).unwrap();
    assert!(quiet["d"].get("blocked").is_none());
    let GatewayMessage::Ready { blocked: none, .. } = serde_json::from_value(quiet).unwrap() else {
        panic!("expected ready");
    };
    assert!(none.is_empty());

    // A client from before blocking knows three kinds and fails on any
    // other one; `ready` still decodes for it, the block is just not seen.
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    enum OldKind {
        Friend,
        Incoming,
        Outgoing,
    }
    #[derive(Deserialize)]
    struct OldRelationship {
        kind: OldKind,
    }
    #[derive(Deserialize)]
    struct OldReady {
        relationships: Vec<OldRelationship>,
    }
    let old: OldReady = serde_json::from_value(json["d"].clone()).unwrap();
    assert_eq!(old.relationships.len(), 1);
    assert!(matches!(old.relationships[0].kind, OldKind::Friend));
    assert!(
        serde_json::from_value::<OldRelationship>(json["d"]["blocked"][0].clone()).is_err(),
        "which is why blocks are not in `relationships`"
    );
    let _ = (OldKind::Incoming, OldKind::Outgoing);
}
