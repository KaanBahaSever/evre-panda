//! Wire-format contract tests: every model, gateway opcode, event and error
//! round-trips through JSON, and key shapes are pinned exactly.

use std::fmt::Debug;

use protocol::error::code;
use protocol::{
    ApiError, Attachment, AudioProfile, Channel, ChannelDelete, ChannelKind, ClientInfo, Device,
    DeviceForm, Event, FieldError, GatewayMessage, Guild, GuildMemberAdd, GuildMemberRemove,
    LimitKey, Member, Message, MessageDelete, OverwriteTarget, PermissionOverwrite, Permissions,
    PlanId, Platform, PresenceStatus, PresenceUpdate, Role, Snowflake, StatusSetting, Timestamp,
    Token, TypingStart, User, VoiceSettings, VoiceState,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

const GUILD: Snowflake = Snowflake::new(100_000_000_000_000_001);
const USER: Snowflake = Snowflake::new(200_000_000_000_000_002);
const CHANNEL: Snowflake = Snowflake::new(300_000_000_000_000_003);
const ROLE: Snowflake = Snowflake::new(400_000_000_000_000_004);
const MESSAGE: Snowflake = Snowflake::new(500_000_000_000_000_005);

fn round_trip<T>(value: &T) -> Value
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let json = serde_json::to_value(value).expect("serialize");
    let text = serde_json::to_string(value).expect("serialize");
    let back: T = serde_json::from_str(&text).expect("deserialize");
    assert_eq!(&back, value, "round trip of {text}");
    json
}

fn user() -> User {
    User {
        id: USER,
        username: "baha".into(),
        display_name: Some("Baha".into()),
        avatar: Some("0123456789abcdef".into()),
        banner: None,
        bio: Some("Selam!\nEra'dayım.".into()),
        bot: false,
        deleted: false,
    }
}

fn guild() -> Guild {
    Guild {
        id: GUILD,
        name: "Kanka".into(),
        owner_id: USER,
        plan: PlanId::Internal,
        icon: Some("fedcba9876543210".into()),
        banner: None,
        access: protocol::GuildAccess::InviteOnly,
        description: None,
        category: None,
        locale: None,
    }
}

fn overwrite() -> PermissionOverwrite {
    PermissionOverwrite {
        target: OverwriteTarget::Role(ROLE),
        allow: Permissions::SEND_MESSAGES,
        deny: Permissions::MENTION_EVERYONE,
    }
}

fn voice_channel() -> Channel {
    Channel {
        id: CHANNEL,
        guild_id: GUILD,
        kind: ChannelKind::Voice,
        name: "Genel Sohbet".into(),
        parent_id: Some(Snowflake::new(7)),
        position: 2,
        topic: None,
        voice: Some(VoiceSettings {
            max_users: Some(15),
            audio_profile: AudioProfile::HdVoice,
            max_bitrate_kbps: 128,
            force_push_to_talk: true,
        }),
        permission_overwrites: vec![
            overwrite(),
            PermissionOverwrite {
                target: OverwriteTarget::Member(USER),
                allow: Permissions::PRIORITY_SPEAKER,
                deny: Permissions::empty(),
            },
        ],
    }
}

fn text_channel() -> Channel {
    Channel {
        id: CHANNEL,
        guild_id: GUILD,
        kind: ChannelKind::Text,
        name: "genel".into(),
        parent_id: None,
        position: 0,
        topic: Some("Hoş geldiniz".into()),
        voice: None,
        permission_overwrites: Vec::new(),
    }
}

fn role() -> Role {
    Role {
        id: ROLE,
        guild_id: GUILD,
        name: "Moderatör".into(),
        color: 0x3B6FD9,
        position: 3,
        permissions: Permissions::MUTE_MEMBERS | Permissions::MOVE_MEMBERS,
        hoist: true,
        mentionable: false,
    }
}

fn member() -> Member {
    Member {
        user_id: USER,
        guild_id: GUILD,
        nick: Some("kanka".into()),
        role_ids: vec![ROLE],
        joined_at: Timestamp::from_unix_millis(1_770_000_000_000),
    }
}

fn message() -> Message {
    Message {
        id: MESSAGE,
        channel_id: CHANNEL,
        author_id: USER,
        content: "selam @everyone".into(),
        nonce: Some("01JAXQ5T4Z8V8Y5W3K9M2N7P6R".into()),
        edited_at: Some(Timestamp::from_unix_millis(1_780_000_000_000)),
        attachments: vec![Attachment {
            id: Snowflake::new(9),
            filename: "ekran.png".into(),
            content_type: Some("image/png".into()),
            size_bytes: 123_456,
            width: Some(1920),
            height: Some(1080),
            url: None,
        }],
        mentions: vec![USER],
        reply: None,
        components: Vec::new(),
        ephemeral: false,
        author: None,
    }
}

fn voice_state() -> VoiceState {
    VoiceState {
        guild_id: GUILD,
        user_id: USER,
        channel_id: Some(CHANNEL),
        self_mute: true,
        self_deaf: false,
        server_mute: false,
        server_deaf: false,
        streaming: true,
        streaming_since: Some(Timestamp::from_unix_millis(1_761_300_000_000)),
        watching: vec![Snowflake::new(43)],
        away: false,
        moved_by: Some(Snowflake::new(42)),
        connection_id: Some(Snowflake::new(44)),
        device: protocol::VoiceDevice::Desktop,
        camera: true,
        camera_source: Some(protocol::CameraSource::Phone),
    }
}

fn all_events() -> Vec<Event> {
    vec![
        Event::MessageCreate(message()),
        Event::MessageUpdate(message()),
        Event::MessageDelete(MessageDelete {
            id: MESSAGE,
            channel_id: CHANNEL,
        }),
        Event::ChannelCreate(voice_channel()),
        Event::ChannelUpdate(text_channel()),
        Event::ChannelDelete(ChannelDelete {
            id: CHANNEL,
            guild_id: GUILD,
        }),
        Event::GuildMemberAdd(GuildMemberAdd {
            member: member(),
            user: user(),
        }),
        Event::GuildMemberRemove(GuildMemberRemove {
            guild_id: GUILD,
            user_id: USER,
        }),
        Event::GuildMemberUpdate(member()),
        Event::VoiceStateUpdate(voice_state()),
        Event::PresenceUpdate(PresenceUpdate {
            user_id: USER,
            status: PresenceStatus::DoNotDisturb,
            version: 7,
            platforms: vec![Platform::Windows, Platform::Ios],
            devices: vec![
                Device {
                    platform: Platform::Windows,
                    form: Some(DeviceForm::Laptop),
                },
                Device {
                    platform: Platform::Ios,
                    form: Some(DeviceForm::Phone),
                },
            ],
        }),
        Event::StatusSettingUpdate(StatusSetting::Invisible),
        Event::UserUpdate(user()),
        Event::TypingStart(TypingStart {
            channel_id: CHANNEL,
            user_id: USER,
        }),
    ]
}

// ---------------------------------------------------------------- models

#[test]
fn models_round_trip() {
    round_trip(&user());
    round_trip(&guild());
    round_trip(&voice_channel());
    round_trip(&text_channel());
    round_trip(&role());
    round_trip(&overwrite());
    round_trip(&member());
    round_trip(&message());
    round_trip(&message().attachments[0]);
    round_trip(&voice_state());
    for status in [
        PresenceStatus::Online,
        PresenceStatus::Idle,
        PresenceStatus::DoNotDisturb,
        PresenceStatus::Offline,
    ] {
        round_trip(&status);
    }
    for setting in [
        StatusSetting::Online,
        StatusSetting::Idle,
        StatusSetting::DoNotDisturb,
        StatusSetting::Invisible,
    ] {
        round_trip(&setting);
    }
    for kind in [
        ChannelKind::Category,
        ChannelKind::Text,
        ChannelKind::Voice,
        ChannelKind::Announcement,
    ] {
        round_trip(&kind);
    }
}

#[test]
fn user_json_shape() {
    assert_eq!(
        round_trip(&user()),
        json!({
            "id": "200000000000000002",
            "username": "baha",
            "displayName": "Baha",
            "avatar": "0123456789abcdef",
            "banner": null,
            "bio": "Selam!\nEra'dayım.",
            "bot": false,
        })
    );
}

#[test]
fn guild_json_shape() {
    assert_eq!(
        round_trip(&guild()),
        json!({
            "id": "100000000000000001",
            "name": "Kanka",
            "ownerId": "200000000000000002",
            "plan": "internal",
            "icon": "fedcba9876543210",
            "banner": null,
            "access": "inviteOnly",
        })
    );
}

#[test]
fn guild_access_and_listing_fields() {
    let mut listed = guild();
    listed.access = protocol::GuildAccess::Request;
    listed.description = Some("Kuzenler burada".into());
    listed.category = Some(protocol::GuildCategory::Gaming);
    listed.locale = Some(protocol::Locale::Tr);
    let json = round_trip(&listed);
    assert_eq!(json["access"], "request");
    assert_eq!(json["description"], "Kuzenler burada");
    assert_eq!(json["category"], "gaming");
    assert_eq!(json["locale"], "tr");
    // An older server's guild is invite-only; unknown modes and categories
    // decode instead of failing.
    let old: Guild = serde_json::from_value(json!({
        "id": "1", "name": "x", "ownerId": "2", "plan": "free", "icon": null, "banner": null
    }))
    .unwrap();
    assert_eq!(old.access, protocol::GuildAccess::InviteOnly);
    let newer: Guild = serde_json::from_value(json!({
        "id": "1", "name": "x", "ownerId": "2", "plan": "free", "icon": null, "banner": null,
        "access": "someday", "category": "cooking"
    }))
    .unwrap();
    assert_eq!(newer.access, protocol::GuildAccess::Unknown);
    assert_eq!(newer.category, Some(protocol::GuildCategory::Unknown));
}

#[test]
fn join_request_events() {
    let count = Event::JoinRequestCreate(protocol::JoinRequestCount {
        guild_id: GUILD,
        count: 3,
    });
    assert_eq!(
        round_trip(&count),
        json!({"t": "joinRequestCreate", "d": {"guildId": "100000000000000001", "count": 3}})
    );
    let update = Event::JoinRequestUpdate(protocol::JoinRequestUpdate {
        guild_id: GUILD,
        guild_name: "Kanka".into(),
        status: protocol::rest::JoinRequestStatus::Rejected,
    });
    assert_eq!(
        round_trip(&update),
        json!({"t": "joinRequestUpdate", "d": {
            "guildId": "100000000000000001", "guildName": "Kanka", "status": "rejected"
        }})
    );
}

#[test]
fn channel_json_shape() {
    assert_eq!(
        round_trip(&voice_channel()),
        json!({
            "id": "300000000000000003",
            "guildId": "100000000000000001",
            "kind": "voice",
            "name": "Genel Sohbet",
            "parentId": "7",
            "position": 2,
            "topic": null,
            "voice": {
                "maxUsers": 15,
                "audioProfile": "hdVoice",
                "maxBitrateKbps": 128,
                "forcePushToTalk": true,
            },
            "permissionOverwrites": [
                {
                    "target": { "type": "role", "id": "400000000000000004" },
                    "allow": "2",
                    "deny": "64",
                },
                {
                    "target": { "type": "member", "id": "200000000000000002" },
                    "allow": "4096",
                    "deny": "0",
                },
            ],
        })
    );
}

#[test]
fn role_and_member_json_shape() {
    assert_eq!(
        round_trip(&role()),
        json!({
            "id": "400000000000000004",
            "guildId": "100000000000000001",
            "name": "Moderatör",
            "color": 0x3B6FD9,
            "position": 3,
            "permissions": "40960",
            "hoist": true,
            "mentionable": false,
        })
    );
    assert_eq!(
        round_trip(&member()),
        json!({
            "userId": "200000000000000002",
            "guildId": "100000000000000001",
            "nick": "kanka",
            "roleIds": ["400000000000000004"],
            "joinedAt": 1_770_000_000_000_u64,
        })
    );
}

#[test]
fn message_json_shape() {
    assert_eq!(
        round_trip(&message()),
        json!({
            "id": "500000000000000005",
            "channelId": "300000000000000003",
            "authorId": "200000000000000002",
            "content": "selam @everyone",
            "nonce": "01JAXQ5T4Z8V8Y5W3K9M2N7P6R",
            "editedAt": 1_780_000_000_000_u64,
            "attachments": [{
                "id": "9",
                "filename": "ekran.png",
                "contentType": "image/png",
                "sizeBytes": 123_456,
                "width": 1920,
                "height": 1080,
            }],
            "mentions": ["200000000000000002"],
        })
    );
    assert_eq!(message().created_at(), MESSAGE.timestamp());
}

#[test]
fn voice_state_json_shape() {
    assert_eq!(
        round_trip(&voice_state()),
        json!({
            "guildId": "100000000000000001",
            "userId": "200000000000000002",
            "channelId": "300000000000000003",
            "selfMute": true,
            "selfDeaf": false,
            "serverMute": false,
            "serverDeaf": false,
            "streaming": true,
            "streamingSince": 1_761_300_000_000_u64,
            "watching": ["43"],
            "away": false,
            "movedBy": "42",
            "connectionId": "44",
            "device": "desktop",
            "camera": true,
            "cameraSource": "phone",
        })
    );
    // A phone's connection without a camera (ADR-076).
    let phone = VoiceState {
        device: protocol::VoiceDevice::Mobile,
        camera: false,
        camera_source: None,
        ..voice_state()
    };
    let json = round_trip(&phone);
    assert_eq!(json["device"], "mobile");
    assert_eq!(json["camera"], false);
    assert!(json.get("cameraSource").is_none());
    // Without a connection ID (older servers, older tokens) the field is
    // left out; older clients ignore it either way (`unknown_fields_are_ignored`).
    let legacy = VoiceState {
        connection_id: None,
        ..voice_state()
    };
    assert!(round_trip(&legacy).get("connectionId").is_none());
    // Not streaming, or from older servers: no start time on the wire.
    let idle = VoiceState {
        streaming: false,
        streaming_since: None,
        ..voice_state()
    };
    assert!(round_trip(&idle).get("streamingSince").is_none());
}

#[test]
fn optional_and_defaulted_fields_may_be_omitted() {
    let user: User = serde_json::from_value(json!({"id": "1", "username": "bot"})).unwrap();
    assert_eq!(user.display_name, None);
    assert_eq!(user.avatar, None);
    assert_eq!(user.banner, None);
    assert_eq!(user.bio, None);
    assert!(!user.bot);
    assert_eq!(user.shown_name(), "bot");

    let state: VoiceState =
        serde_json::from_value(json!({"guildId": "1", "userId": "2", "channelId": null})).unwrap();
    assert_eq!(state.channel_id, None);
    assert!(!state.self_mute && !state.streaming);
    assert_eq!(state.moved_by, None);
    assert_eq!(state.connection_id, None);
    assert_eq!(state.streaming_since, None);
    // Older servers: a desktop connection without a camera.
    assert_eq!(state.device, protocol::VoiceDevice::Desktop);
    assert!(!state.camera && state.camera_source.is_none());

    let member: Member =
        serde_json::from_value(json!({"userId": "1", "guildId": "2", "joinedAt": 0})).unwrap();
    assert!(member.role_ids.is_empty());

    // Servers before profile images sent no icon or banner.
    let guild: Guild = serde_json::from_value(json!({
        "id": "1",
        "name": "Eski",
        "ownerId": "2",
        "plan": "free",
    }))
    .unwrap();
    assert_eq!((guild.icon, guild.banner), (None, None));
}

#[test]
fn unknown_fields_are_ignored() {
    let user: User = serde_json::from_value(json!({
        "id": "1",
        "username": "baha",
        "pronouns": "he/him",
    }))
    .unwrap();
    assert_eq!(user.username, "baha");
}

#[test]
fn ids_must_be_strings() {
    assert!(serde_json::from_value::<User>(json!({"id": 1, "username": "x"})).is_err());
}

#[test]
fn unknown_channel_kind_decodes_as_unknown() {
    let mut json = serde_json::to_value(text_channel()).unwrap();
    json["kind"] = json!("forum");
    let channel: Channel = serde_json::from_value(json).unwrap();
    assert_eq!(channel.kind, ChannelKind::Unknown);
}

#[test]
fn model_helpers() {
    assert_eq!(guild().everyone_role_id(), GUILD);
    assert_eq!(
        user().avatar_path().as_deref(),
        Some("/v1/media/avatars/200000000000000002/0123456789abcdef")
    );
    assert_eq!(user().banner_path(), None);
    assert_eq!(
        guild().icon_path().as_deref(),
        Some("/v1/media/icons/100000000000000001/fedcba9876543210")
    );
    assert_eq!(guild().banner_path(), None);
    assert!(!role().is_everyone());
    let everyone = Role {
        id: GUILD,
        ..role()
    };
    assert!(everyone.is_everyone());
    assert_eq!(AudioProfile::Voice.bitrate_range_kbps(), 32..=128);
    assert_eq!(AudioProfile::Studio.bitrate_range_kbps(), 128..=256);
}

// ---------------------------------------------------------------- gateway

#[test]
fn every_opcode_round_trips() {
    let messages = vec![
        GatewayMessage::Hello {
            heartbeat_interval_ms: 41_250,
        },
        GatewayMessage::Identify {
            token: Token::new("tok"),
            idle: true,
            client: Some(ClientInfo {
                os: Platform::MacOs,
                version: Some("0.1.19".into()),
                form: Some(DeviceForm::Laptop),
                locale: Some("de-DE".into()),
            }),
        },
        GatewayMessage::Ready {
            session_id: "s-1".into(),
            user: user(),
            guilds: vec![guild()],
            status: StatusSetting::DoNotDisturb,
            dms: Vec::new(),
            relationships: Vec::new(),
            blocked: Vec::new(),
            read_states: Vec::new(),
        },
        GatewayMessage::Dispatch {
            seq: 1,
            event: Event::TypingStart(TypingStart {
                channel_id: CHANNEL,
                user_id: USER,
            }),
        },
        GatewayMessage::Heartbeat { last_seq: 0 },
        GatewayMessage::HeartbeatAck,
        GatewayMessage::Resume {
            token: Token::new("tok"),
            session_id: "s-1".into(),
            last_seq: 17,
        },
        GatewayMessage::Resumed,
        GatewayMessage::UpdatePresence {
            idle: Some(true),
            status: None,
        },
        GatewayMessage::Reconnect,
        GatewayMessage::InvalidSession { resumable: false },
    ];
    let ops: Vec<String> = messages
        .iter()
        .map(|message| round_trip(message)["op"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        ops,
        [
            "hello",
            "identify",
            "ready",
            "dispatch",
            "heartbeat",
            "heartbeatAck",
            "resume",
            "resumed",
            "updatePresence",
            "reconnect",
            "invalidSession",
        ]
    );
}

#[test]
fn opcode_json_shapes() {
    assert_eq!(
        round_trip(&GatewayMessage::Hello {
            heartbeat_interval_ms: 41_250
        }),
        json!({"op": "hello", "d": {"heartbeatIntervalMs": 41_250}})
    );
    assert_eq!(
        round_trip(&GatewayMessage::HeartbeatAck),
        json!({"op": "heartbeatAck"})
    );
    assert_eq!(
        round_trip(&GatewayMessage::Resume {
            token: Token::new("tok"),
            session_id: "s-1".into(),
            last_seq: 17,
        }),
        json!({"op": "resume", "d": {"token": "tok", "sessionId": "s-1", "lastSeq": 17}})
    );
    assert_eq!(
        round_trip(&GatewayMessage::InvalidSession { resumable: true }),
        json!({"op": "invalidSession", "d": {"resumable": true}})
    );
    assert_eq!(
        round_trip(&GatewayMessage::UpdatePresence {
            idle: None,
            status: Some(StatusSetting::DoNotDisturb),
        }),
        json!({"op": "updatePresence", "d": {"status": "doNotDisturb"}})
    );
    // Older clients identify without `idle` and `client`; older servers send
    // no `status`.
    let identify: GatewayMessage =
        serde_json::from_value(json!({"op": "identify", "d": {"token": "tok"}})).unwrap();
    assert_eq!(
        identify,
        GatewayMessage::Identify {
            token: Token::new("tok"),
            idle: false,
            client: None,
        }
    );
    assert_eq!(
        round_trip(&GatewayMessage::Identify {
            token: Token::new("tok"),
            idle: false,
            client: Some(ClientInfo {
                os: Platform::Android,
                version: None,
                form: None,
                locale: None,
            }),
        }),
        json!({"op": "identify", "d": {"token": "tok", "idle": false, "client": {"os": "android"}}})
    );
    assert_eq!(
        round_trip(&GatewayMessage::Identify {
            token: Token::new("tok"),
            idle: false,
            client: Some(ClientInfo {
                os: Platform::Windows,
                version: None,
                form: Some(DeviceForm::Laptop),
                locale: None,
            }),
        })["d"]["client"],
        json!({"os": "windows", "form": "laptop"})
    );
    // The app's language (ADR-083): a tag, so one this build does not know
    // never fails the identify.
    assert_eq!(
        round_trip(&GatewayMessage::Identify {
            token: Token::new("tok"),
            idle: false,
            client: Some(ClientInfo {
                os: Platform::Ios,
                version: None,
                form: None,
                locale: Some("ja-JP".into()),
            }),
        })["d"]["client"],
        json!({"os": "ios", "locale": "ja-JP"})
    );
    // A platform from a newer client is kept as unknown, not refused.
    let identify: GatewayMessage = serde_json::from_value(
        json!({"op": "identify", "d": {"token": "tok", "client": {"os": "web", "version": "2.0"}}}),
    )
    .unwrap();
    assert!(matches!(
        identify,
        GatewayMessage::Identify {
            client: Some(ClientInfo {
                os: Platform::Unknown,
                ..
            }),
            ..
        }
    ));
    // So is a form from a newer client.
    let identify: GatewayMessage = serde_json::from_value(
        json!({"op": "identify", "d": {"token": "tok", "client": {"os": "linux", "form": "watch"}}}),
    )
    .unwrap();
    assert!(matches!(
        identify,
        GatewayMessage::Identify {
            client: Some(ClientInfo {
                os: Platform::Linux,
                form: Some(DeviceForm::Unknown),
                ..
            }),
            ..
        }
    ));
}

#[test]
fn presence_platforms_are_optional() {
    let update = PresenceUpdate {
        user_id: USER,
        status: PresenceStatus::Online,
        version: 9,
        platforms: Vec::new(),
        devices: Vec::new(),
    };
    // No platforms: the field is left out, as older servers send it.
    let json = round_trip(&update);
    assert_eq!(
        json,
        json!({"userId": "200000000000000002", "status": "online", "version": 9})
    );
    let both = PresenceUpdate {
        platforms: vec![Platform::Windows, Platform::Android],
        ..update
    };
    assert_eq!(
        round_trip(&both)["platforms"],
        json!(["windows", "android"])
    );
    // Platforms from a newer server decode as unknown.
    let newer: PresenceUpdate = serde_json::from_value(json!({
        "userId": "200000000000000002", "status": "idle", "version": 10,
        "platforms": ["linux", "vr"]
    }))
    .unwrap();
    assert_eq!(newer.platforms, [Platform::Linux, Platform::Unknown]);
    assert!(newer.devices.is_empty(), "an older server sends no devices");
}

#[test]
fn presence_devices_carry_their_forms() {
    let update = PresenceUpdate {
        user_id: USER,
        status: PresenceStatus::Online,
        version: 9,
        platforms: vec![Platform::MacOs, Platform::Ios],
        devices: vec![
            Device {
                platform: Platform::MacOs,
                form: Some(DeviceForm::Laptop),
            },
            Device {
                platform: Platform::Ios,
                form: None,
            },
        ],
    };
    assert_eq!(
        round_trip(&update),
        json!({
            "userId": "200000000000000002", "status": "online", "version": 9,
            "platforms": ["macos", "ios"],
            "devices": [{"platform": "macos", "form": "laptop"}, {"platform": "ios"}]
        })
    );
    // Platforms and forms from a newer server decode as unknown.
    let newer: PresenceUpdate = serde_json::from_value(json!({
        "userId": "200000000000000002", "status": "online", "version": 10,
        "devices": [{"platform": "vr", "form": "headset"}, {"platform": "android", "form": "phone"}]
    }))
    .unwrap();
    assert_eq!(
        newer.devices,
        [
            Device {
                platform: Platform::Unknown,
                form: Some(DeviceForm::Unknown),
            },
            Device {
                platform: Platform::Android,
                form: Some(DeviceForm::Phone),
            },
        ]
    );
}

#[test]
fn unit_opcodes_accept_null_data() {
    let message: GatewayMessage =
        serde_json::from_value(json!({"op": "reconnect", "d": null})).unwrap();
    assert_eq!(message, GatewayMessage::Reconnect);
}

#[test]
fn every_event_round_trips_inside_a_dispatch() {
    let events = all_events();
    for (seq, event) in (1..).zip(events.iter()) {
        let dispatch = GatewayMessage::Dispatch {
            seq,
            event: event.clone(),
        };
        let json = round_trip(&dispatch);
        assert_eq!(json["d"]["seq"], seq);
        assert_eq!(json["d"]["event"]["t"], event.event_type());
        round_trip(event);
    }
    let mut types: Vec<_> = events.iter().map(Event::event_type).collect();
    types.sort_unstable();
    types.dedup();
    assert_eq!(types.len(), 14, "every event type is distinct");
}

#[test]
fn dispatch_json_shape() {
    let dispatch = GatewayMessage::Dispatch {
        seq: 5,
        event: Event::MessageDelete(MessageDelete {
            id: MESSAGE,
            channel_id: CHANNEL,
        }),
    };
    assert_eq!(
        round_trip(&dispatch),
        json!({
            "op": "dispatch",
            "d": {
                "seq": 5,
                "event": {
                    "t": "messageDelete",
                    "d": {"id": "500000000000000005", "channelId": "300000000000000003"},
                },
            },
        })
    );
}

#[test]
fn unknown_event_types_decode_as_unknown() {
    for event in [
        json!({"t": "threadCreate", "d": {"id": "1", "nested": [1, 2, 3]}}),
        json!({"d": {"id": "1"}, "t": "threadCreate"}),
        json!({"t": "threadCreate"}),
        json!({"t": "threadCreate", "d": null}),
    ] {
        let message: GatewayMessage =
            serde_json::from_value(json!({"op": "dispatch", "d": {"seq": 9, "event": event}}))
                .unwrap();
        let GatewayMessage::Dispatch { seq, event } = message else {
            panic!("expected a dispatch");
        };
        assert_eq!(seq, 9);
        assert_eq!(
            event,
            Event::Unknown {
                event_type: "threadCreate".into()
            }
        );
        assert_eq!(event.event_type(), "threadCreate");
    }
}

#[test]
fn known_events_tolerate_any_key_order() {
    let message: GatewayMessage = serde_json::from_value(json!({
        "d": {"event": {"d": {"channelId": "3", "userId": "2"}, "t": "typingStart"}, "seq": 1},
        "op": "dispatch",
    }))
    .unwrap();
    assert_eq!(
        message,
        GatewayMessage::Dispatch {
            seq: 1,
            event: Event::TypingStart(TypingStart {
                channel_id: Snowflake::new(3),
                user_id: Snowflake::new(2),
            }),
        }
    );
}

#[test]
fn malformed_known_events_are_errors() {
    let result = serde_json::from_value::<GatewayMessage>(json!({
        "op": "dispatch",
        "d": {"seq": 1, "event": {"t": "typingStart", "d": {"channelId": "3"}}},
    }));
    let error = result.unwrap_err().to_string();
    assert!(error.contains("userId"), "{error}");
}

#[test]
fn unknown_events_are_never_encoded() {
    let event = Event::Unknown {
        event_type: "threadCreate".into(),
    };
    assert!(serde_json::to_string(&event).is_err());
}

#[test]
fn unknown_opcodes_are_errors() {
    assert!(serde_json::from_value::<GatewayMessage>(json!({"op": "requestMembers"})).is_err());
}

#[test]
fn gateway_version() {
    assert_eq!(protocol::GATEWAY_VERSION, 1);
}

// ---------------------------------------------------------------- errors

#[test]
fn every_error_round_trips() {
    let errors = vec![
        ApiError::Validation { errors: vec![] },
        ApiError::Validation {
            errors: vec![FieldError {
                field: "name".into(),
                message: "too long".into(),
            }],
        },
        ApiError::Unauthorized,
        ApiError::Forbidden,
        ApiError::PlanLimitReached {
            key: LimitKey::MaxConcurrentStreams,
        },
        ApiError::NotFound,
        ApiError::RateLimited {
            retry_after_ms: 1_500,
        },
        ApiError::EmailNotVerified {
            verification: Box::new(protocol::rest::EmailVerification {
                ticket: None,
                email: "k***@g***.com".into(),
                expires_at: protocol::Timestamp::from_unix_millis(5),
                resend_after_ms: 0,
                verified: false,
            }),
        },
        ApiError::Internal,
        ApiError::Unknown {
            code: 40_901,
            message: "conflict".into(),
        },
    ];
    for error in &errors {
        let json = round_trip(error);
        assert_eq!(json["code"], error.code());
        assert!(json["message"].is_string());
    }
}

#[test]
fn error_json_shapes() {
    assert_eq!(
        round_trip(&ApiError::NotFound),
        json!({"code": 40_400, "message": "not found"})
    );
    assert_eq!(
        round_trip(&ApiError::RateLimited {
            retry_after_ms: 1_500
        }),
        json!({
            "code": 42_900,
            "message": "rate limited, retry after 1500 ms",
            "retryAfterMs": 1_500,
        })
    );
    assert_eq!(
        round_trip(&ApiError::PlanLimitReached {
            key: LimitKey::MaxFileSizeBytes
        }),
        json!({
            "code": 40_301,
            "message": "plan limit reached: max_file_size_bytes",
            "key": "max_file_size_bytes",
        })
    );
    assert_eq!(
        round_trip(&ApiError::validation("name", "too long")),
        json!({
            "code": 40_000,
            "message": "invalid request",
            "errors": [{"field": "name", "message": "too long"}],
        })
    );
}

#[test]
fn incomplete_or_new_errors_decode_as_unknown() {
    let cases = [
        json!({"code": 40_301, "message": "limit", "key": "max_spaceships"}),
        json!({"code": 40_301, "message": "limit"}),
        json!({"code": 42_900, "message": "slow down"}),
        json!({"code": 45_100, "message": "unavailable for legal reasons"}),
    ];
    for json in cases {
        let code = u32::try_from(json["code"].as_u64().unwrap()).unwrap();
        let message = json["message"].as_str().unwrap().to_owned();
        let error: ApiError = serde_json::from_value(json).unwrap();
        assert_eq!(error, ApiError::Unknown { code, message });
    }
    assert_eq!(code::PLAN_LIMIT_REACHED, 40_301);
}
