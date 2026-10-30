//! Wire-format tests for the REST bodies and the events added with
//! era-server 0.1.

use std::fmt::Debug;

use protocol::gateway::close_code;
use protocol::rest::media::{IMAGE_TYPES, MEDIA_HASH_LEN, is_media_hash};
use protocol::rest::{
    AuthResponse, ChannelPermissions, CreateChannelRequest, CreateDmRequest, GuildMember,
    GuildSnapshot, IceServer, LoginRequest, MediaKind, MessageQuery, Password, RegisterRequest,
    UpdateChannelRequest, UpdateInfo, UpdateMeRequest, UpdateVoiceStateRequest, VoiceGrants,
    VoiceJoinResponse, VoiceSource, limits, media_path,
};
use protocol::{
    AudioProfile, ChannelKind, DmChannel, DmDelete, Event, GatewayMessage, Guild, GuildRoleDelete,
    Invite, Member, Permissions, PlanId, PresenceStatus, PresenceUpdate, Role, Snowflake,
    StatusSetting, Timestamp, Token, User,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

const GUILD: Snowflake = Snowflake::new(100);
const USER: Snowflake = Snowflake::new(200);
const CHANNEL: Snowflake = Snowflake::new(300);

fn round_trip<T>(value: &T) -> Value
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let text = serde_json::to_string(value).expect("serialize");
    let back: T = serde_json::from_str(&text).expect("deserialize");
    assert_eq!(&back, value, "round trip of {text}");
    serde_json::from_str(&text).unwrap()
}

fn user() -> User {
    User {
        id: USER,
        username: "baha".into(),
        display_name: None,
        avatar: None,
        banner: None,
        bio: None,
        bot: false,
        deleted: false,
    }
}

fn guild() -> Guild {
    Guild {
        id: GUILD,
        name: "Bizim Sunucu".into(),
        owner_id: USER,
        plan: PlanId::Internal,
        icon: None,
        banner: Some("0123456789abcdef".into()),
        access: protocol::GuildAccess::InviteOnly,
        description: None,
        category: None,
        locale: None,
    }
}

fn everyone() -> Role {
    Role {
        id: GUILD,
        guild_id: GUILD,
        name: "@everyone".into(),
        color: 0,
        position: 0,
        permissions: Permissions::VIEW_CHANNEL,
        hoist: false,
        mentionable: false,
    }
}

fn snapshot() -> GuildSnapshot {
    GuildSnapshot {
        guild: guild(),
        channels: Vec::new(),
        roles: vec![everyone()],
        members: vec![GuildMember {
            member: Member {
                user_id: USER,
                guild_id: GUILD,
                nick: None,
                role_ids: Vec::new(),
                joined_at: Timestamp::from_unix_millis(1),
            },
            user: user(),
        }],
        voice_states: Vec::new(),
        permissions: Permissions::all(),
        channel_permissions: vec![ChannelPermissions {
            channel_id: CHANNEL,
            permissions: Permissions::VIEW_CHANNEL,
        }],
        presences: vec![PresenceUpdate {
            user_id: USER,
            status: PresenceStatus::Online,
            version: 3,
            platforms: vec![protocol::Platform::Linux],
            devices: vec![protocol::Device {
                platform: protocol::Platform::Linux,
                form: Some(protocol::DeviceForm::Laptop),
            }],
        }],
        presence_version: 5,
        read_states: Vec::new(),
        open_reports: 0,
        pending_join_requests: 0,
    }
}

#[test]
fn passwords_are_redacted_and_transparent() {
    let request = LoginRequest {
        username: "baha".into(),
        password: Password::new("hunter2hunter2"),
    };
    let debug = format!("{request:?}");
    assert!(!debug.contains("hunter2"), "{debug}");
    assert_eq!(
        round_trip(&request),
        json!({"username": "baha", "password": "hunter2hunter2"})
    );
    assert_eq!(request.password.expose(), "hunter2hunter2");
}

#[test]
fn register_request_shape() {
    let request: RegisterRequest = serde_json::from_value(json!({
        "username": "kanka",
        "password": "correct horse",
        "inviteCode": "abc123",
    }))
    .unwrap();
    assert_eq!(request.display_name, None);
    assert_eq!(request.invite_code.as_deref(), Some("abc123"));
    assert_eq!(request.email, None);
    round_trip(&request);

    // Open registration: no code, an email.
    let request: RegisterRequest = serde_json::from_value(json!({
        "username": "kanka",
        "email": "kanka@example.com",
        "password": "correct horse",
    }))
    .unwrap();
    assert_eq!(request.invite_code, None);
    assert_eq!(
        round_trip(&request),
        json!({"username": "kanka", "email": "kanka@example.com", "password": "correct horse", "displayName": null})
    );
}

#[test]
fn auth_response_shape() {
    let response = AuthResponse {
        token: Token::new("era_secret"),
        user: user(),
        expires_at: Timestamp::from_unix_millis(5),
    };
    let json = round_trip(&response);
    assert_eq!(json["token"], "era_secret");
    assert_eq!(json["expiresAt"], 5);
    assert!(!format!("{response:?}").contains("era_secret"));
}

#[test]
fn update_channel_distinguishes_absent_and_null() {
    let absent: UpdateChannelRequest = serde_json::from_value(json!({"name": "x"})).unwrap();
    assert_eq!(absent.parent_id, None);
    assert_eq!(absent.topic, None);

    let cleared: UpdateChannelRequest =
        serde_json::from_value(json!({"parentId": null, "topic": null})).unwrap();
    assert_eq!(cleared.parent_id, Some(None));
    assert_eq!(cleared.topic, Some(None));

    let set: UpdateChannelRequest =
        serde_json::from_value(json!({"parentId": "7", "topic": "t"})).unwrap();
    assert_eq!(set.parent_id, Some(Some(Snowflake::new(7))));
    assert_eq!(set.topic, Some(Some("t".into())));

    // Absent fields are not serialized; explicit nulls are.
    assert_eq!(round_trip(&absent), json!({"name": "x"}));
    assert_eq!(
        round_trip(&cleared),
        json!({"parentId": null, "topic": null})
    );
}

#[test]
fn update_me_distinguishes_absent_and_null() {
    let empty: UpdateMeRequest = serde_json::from_value(json!({})).unwrap();
    assert_eq!(empty, UpdateMeRequest::default());
    assert!(!empty.clear_avatar && !empty.clear_banner);
    assert_eq!(round_trip(&empty), json!({}));

    let cleared: UpdateMeRequest = serde_json::from_value(json!({
        "displayName": null,
        "bio": null,
        "clearAvatar": true,
    }))
    .unwrap();
    assert_eq!(cleared.display_name, Some(None));
    assert_eq!(cleared.bio, Some(None));
    assert!(cleared.clear_avatar && !cleared.clear_banner);
    assert_eq!(
        round_trip(&cleared),
        json!({"displayName": null, "bio": null, "clearAvatar": true})
    );

    let set: UpdateMeRequest =
        serde_json::from_value(json!({"displayName": "Baha", "bio": "selam"})).unwrap();
    assert_eq!(set.display_name, Some(Some("Baha".into())));
    assert_eq!(set.bio, Some(Some("selam".into())));
}

#[test]
fn media_paths_and_kinds() {
    let segments: Vec<_> = MediaKind::ALL.iter().map(|kind| kind.as_str()).collect();
    assert_eq!(segments, ["avatars", "banners", "icons", "guild-banners"]);
    for kind in MediaKind::ALL {
        assert_eq!(kind.as_str().parse::<MediaKind>(), Ok(kind));
    }
    assert!("avatar".parse::<MediaKind>().is_err());
    assert!("..".parse::<MediaKind>().is_err());
    assert_eq!(MediaKind::Avatar.max_bytes(), limits::MAX_AVATAR_BYTES);
    assert_eq!(MediaKind::GuildIcon.max_bytes(), limits::MAX_AVATAR_BYTES);
    assert_eq!(MediaKind::Banner.max_bytes(), limits::MAX_BANNER_BYTES);
    assert_eq!(MediaKind::GuildBanner.max_bytes(), limits::MAX_BANNER_BYTES);
    assert_eq!(
        media_path(MediaKind::GuildBanner, GUILD, "0123456789abcdef"),
        "/v1/media/guild-banners/100/0123456789abcdef"
    );
    assert!(is_media_hash("0123456789abcdef"));
    for bad in [
        "",
        "0123456789ABCDEF",
        "0123456789abcde",
        "0123456789abcdefa",
        "../../etc/passwd",
    ] {
        assert!(!is_media_hash(bad), "{bad}");
    }
    assert_eq!(MEDIA_HASH_LEN, 16);
    assert_eq!(IMAGE_TYPES.len(), 4);
}

#[test]
fn create_channel_defaults() {
    let request: CreateChannelRequest =
        serde_json::from_value(json!({"kind": "voice", "name": "Lobi"})).unwrap();
    assert_eq!(request.kind, ChannelKind::Voice);
    assert_eq!(request.voice, None);
    assert_eq!(request.permission_overwrites, None);
    round_trip(&request);
}

#[test]
fn message_query_is_query_string_friendly() {
    let query: MessageQuery =
        serde_json::from_value(json!({"before": "123", "limit": 10})).unwrap();
    assert_eq!(query.before, Some(Snowflake::new(123)));
    assert_eq!(query.after, None);
    assert_eq!(round_trip(&query), json!({"before": "123", "limit": 10}));
}

#[test]
fn voice_sources_map_to_livekit_names() {
    let names: Vec<_> = [
        VoiceSource::Microphone,
        VoiceSource::ScreenShare,
        VoiceSource::ScreenShareAudio,
    ]
    .iter()
    .map(|source| (round_trip(source), source.livekit_name()))
    .collect();
    assert_eq!(
        names,
        [
            (json!("microphone"), "microphone"),
            (json!("screenShare"), "screen_share"),
            (json!("screenShareAudio"), "screen_share_audio"),
        ]
    );
}

#[test]
fn voice_join_response_shape() {
    let response = VoiceJoinResponse {
        url: "wss://media.example".into(),
        token: Token::new("jwt"),
        room: "v_100_300".into(),
        identity: "200".into(),
        expires_at: Timestamp::from_unix_millis(9),
        ice_servers: vec![IceServer {
            urls: vec!["turns:turn.example:443".into()],
            username: None,
            credential: None,
        }],
        grants: VoiceGrants {
            can_subscribe: true,
            can_publish_sources: vec![VoiceSource::Microphone],
            max_bitrate_kbps: 64,
            audio_profile: AudioProfile::Voice,
            force_push_to_talk: false,
            max_music_bitrate_kbps: None,
            can_publish_camera: false,
        },
        connection_id: Some(Snowflake::new(77)),
    };
    assert_eq!(
        round_trip(&response),
        json!({
            "url": "wss://media.example",
            "token": "jwt",
            "room": "v_100_300",
            "identity": "200",
            "expiresAt": 9,
            "iceServers": [{"urls": ["turns:turn.example:443"], "username": null, "credential": null}],
            "grants": {
                "canSubscribe": true,
                "canPublishSources": ["microphone"],
                "maxBitrateKbps": 64,
                "audioProfile": "voice",
                "forcePushToTalk": false,
            },
            "connectionId": "77",
        })
    );
    // A desktop that may publish its camera says so in a field of its own,
    // so older clients (which reject unknown sources) still read the grants.
    let mut camera = response.clone();
    camera.grants.can_publish_camera = true;
    let json = round_trip(&camera);
    assert_eq!(json["grants"]["canPublishCamera"], true);
    assert_eq!(json["grants"]["canPublishSources"], json!(["microphone"]));
    // The join request's takeover flag (ADR-076) is left out when false.
    assert_eq!(
        serde_json::to_value(protocol::rest::VoiceJoinRequest::default()).unwrap(),
        json!({"selfMute": false, "selfDeaf": false})
    );
    assert_eq!(
        serde_json::to_value(protocol::rest::VoiceJoinRequest {
            replace_other_device: true,
            ..protocol::rest::VoiceJoinRequest::default()
        })
        .unwrap()["replaceOtherDevice"],
        true
    );
    // Older servers send no connection ID.
    let mut old_response = serde_json::to_value(&response).unwrap();
    old_response.as_object_mut().unwrap().remove("connectionId");
    let decoded: VoiceJoinResponse = serde_json::from_value(old_response).unwrap();
    assert_eq!(decoded.connection_id, None);

    // A bot's grants also carry the plan's music bitrate.
    let mut bot = response.grants.clone();
    bot.max_music_bitrate_kbps = Some(192);
    let value = round_trip(&bot);
    assert_eq!(value["maxMusicBitrateKbps"], 192);
    // Older servers send no such field.
    let old: VoiceGrants = serde_json::from_value(json!({
        "canSubscribe": true,
        "canPublishSources": [],
        "maxBitrateKbps": 64,
        "audioProfile": "voice",
        "forcePushToTalk": false,
    }))
    .unwrap();
    assert_eq!(old.max_music_bitrate_kbps, None);
}

#[test]
fn voice_state_updates_name_their_connection() {
    let request = UpdateVoiceStateRequest {
        self_mute: Some(true),
        connection_id: Some(Snowflake::new(77)),
        ..UpdateVoiceStateRequest::default()
    };
    assert_eq!(
        round_trip(&request),
        json!({"selfMute": true, "connectionId": "77"})
    );
    // Older clients send no connection ID.
    let old: UpdateVoiceStateRequest = serde_json::from_value(json!({"away": true})).unwrap();
    assert_eq!(old.connection_id, None);
}

#[test]
fn update_info_uses_the_tauri_shape() {
    let info = UpdateInfo {
        version: "0.1.3".into(),
        notes: None,
        pub_date: Some("2026-10-21T12:00:00Z".into()),
        url: "https://cdn.example/Era_0.1.3_x64-setup.exe".into(),
        signature: "untrusted comment: …".into(),
        min_supported_version: Some("0.1.0".into()),
    };
    assert_eq!(
        round_trip(&info),
        json!({
            "version": "0.1.3",
            "pub_date": "2026-10-21T12:00:00Z",
            "url": "https://cdn.example/Era_0.1.3_x64-setup.exe",
            "signature": "untrusted comment: …",
            "minSupportedVersion": "0.1.0",
        })
    );
}

#[test]
fn invite_shape() {
    let invite = Invite {
        code: "Ab3dE9xYz1".into(),
        guild_id: GUILD,
        channel_id: None,
        inviter_id: Some(USER),
        max_uses: None,
        uses: 2,
        expires_at: None,
        created_at: Timestamp::from_unix_millis(3),
    };
    assert_eq!(
        round_trip(&invite),
        json!({
            "code": "Ab3dE9xYz1",
            "guildId": "100",
            "channelId": null,
            "inviterId": "200",
            "maxUses": null,
            "uses": 2,
            "expiresAt": null,
            "createdAt": 3,
        })
    );
}

#[test]
fn new_events_round_trip_inside_a_dispatch() {
    let events = [
        Event::GuildCreate(snapshot()),
        Event::GuildUpdate(guild()),
        Event::GuildRoleCreate(everyone()),
        Event::GuildRoleUpdate(everyone()),
        Event::GuildRoleDelete(GuildRoleDelete {
            guild_id: GUILD,
            role_id: Snowflake::new(5),
        }),
        Event::UserUpdate(user()),
    ];
    let names: Vec<_> = events.iter().map(Event::event_type).collect();
    assert_eq!(
        names,
        [
            "guildCreate",
            "guildUpdate",
            "guildRoleCreate",
            "guildRoleUpdate",
            "guildRoleDelete",
            "userUpdate",
        ]
    );
    for (seq, event) in (1..).zip(events) {
        let json = round_trip(&GatewayMessage::Dispatch {
            seq,
            event: event.clone(),
        });
        assert_eq!(json["d"]["event"]["t"], event.event_type());
    }
}

#[test]
fn user_update_shape() {
    let user = User {
        avatar: Some("0123456789abcdef".into()),
        bio: Some("selam".into()),
        ..user()
    };
    assert_eq!(
        round_trip(&Event::UserUpdate(user)),
        json!({
            "t": "userUpdate",
            "d": {
                "id": "200",
                "username": "baha",
                "displayName": null,
                "avatar": "0123456789abcdef",
                "banner": null,
                "bio": "selam",
                "bot": false,
            },
        })
    );
    assert_eq!(
        round_trip(&Event::GuildUpdate(guild()))["d"]["banner"],
        "0123456789abcdef"
    );
}

#[test]
fn guild_create_shape() {
    let json = round_trip(&Event::GuildCreate(snapshot()));
    assert_eq!(json["t"], "guildCreate");
    let d = &json["d"];
    assert_eq!(d["guild"]["id"], "100");
    assert_eq!(d["members"][0]["member"]["userId"], "200");
    assert_eq!(d["members"][0]["user"]["username"], "baha");
    assert_eq!(d["permissions"], Permissions::all().bits().to_string());
    assert_eq!(
        d["channelPermissions"],
        json!([{"channelId": "300", "permissions": "1"}])
    );
    assert_eq!(d["voiceStates"], json!([]));
}

#[test]
fn close_codes_are_distinct_application_codes() {
    let codes = [
        close_code::UNKNOWN_ERROR,
        close_code::UNKNOWN_OPCODE,
        close_code::DECODE_ERROR,
        close_code::NOT_AUTHENTICATED,
        close_code::AUTHENTICATION_FAILED,
        close_code::ALREADY_AUTHENTICATED,
        close_code::RATE_LIMITED,
        close_code::SESSION_TIMED_OUT,
        close_code::INVALID_VERSION,
    ];
    let mut sorted = codes.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), codes.len());
    assert!(codes.iter().all(|code| (4000..5000).contains(code)));
}

fn group_dm() -> DmChannel {
    DmChannel {
        id: CHANNEL,
        recipients: vec![
            user(),
            User {
                id: Snowflake::new(201),
                username: "kuzen".into(),
                ..user()
            },
        ],
        owner_id: Some(USER),
        name: Some("Kuzenler".into()),
        last_message_id: Some(Snowflake::new(900)),
    }
}

#[test]
fn dm_channel_shape() {
    let json = round_trip(&group_dm());
    assert_eq!(json["id"], "300");
    assert_eq!(json["recipients"][1]["username"], "kuzen");
    assert_eq!(json["ownerId"], "200");
    assert_eq!(json["name"], "Kuzenler");
    assert_eq!(json["lastMessageId"], "900");
    assert!(group_dm().is_group());

    // A 1:1 DM without messages: no owner, no name; optional fields may be
    // omitted on input.
    let direct: DmChannel = serde_json::from_value(json!({
        "id": "301",
        "recipients": [json["recipients"][0], json["recipients"][1]],
    }))
    .unwrap();
    assert_eq!(direct.owner_id, None);
    assert_eq!(direct.name, None);
    assert_eq!(direct.last_message_id, None);
    assert!(!direct.is_group());
    assert_eq!(
        round_trip(&direct)["ownerId"],
        Value::Null,
        "absent options serialize as null"
    );
}

#[test]
fn create_dm_request_shape() {
    let direct = CreateDmRequest {
        recipient_ids: vec![USER],
        name: None,
    };
    assert_eq!(round_trip(&direct), json!({"recipientIds": ["200"]}));
    let group = CreateDmRequest {
        recipient_ids: vec![USER, Snowflake::new(201)],
        name: Some("Kuzenler".into()),
    };
    assert_eq!(
        round_trip(&group),
        json!({"recipientIds": ["200", "201"], "name": "Kuzenler"})
    );
    assert_eq!(limits::MAX_DM_RECIPIENTS, 10);
    assert_eq!(*limits::DM_NAME_LEN.end(), 100);
}

#[test]
fn dm_events_round_trip_inside_a_dispatch() {
    let events = [
        Event::DmCreate(group_dm()),
        Event::DmUpdate(group_dm()),
        Event::DmDelete(DmDelete { id: CHANNEL }),
    ];
    let names: Vec<_> = events.iter().map(Event::event_type).collect();
    assert_eq!(names, ["dmCreate", "dmUpdate", "dmDelete"]);
    for (seq, event) in (1..).zip(events) {
        let json = round_trip(&GatewayMessage::Dispatch {
            seq,
            event: event.clone(),
        });
        assert_eq!(json["d"]["event"]["t"], event.event_type());
    }
    assert_eq!(
        round_trip(&Event::DmDelete(DmDelete { id: CHANNEL })),
        json!({"t": "dmDelete", "d": {"id": "300"}})
    );
}

#[test]
fn ready_carries_dms_and_decodes_without_them() {
    let ready = GatewayMessage::Ready {
        session_id: "s-1".into(),
        user: user(),
        guilds: vec![guild()],
        status: StatusSetting::Idle,
        dms: vec![group_dm()],
        relationships: vec![protocol::Relationship {
            user: user(),
            kind: protocol::RelationshipKind::Friend,
            since: protocol::Timestamp::from_unix_millis(1_700_000_000_000),
            presence: Some(protocol::PresenceUpdate {
                user_id: user().id,
                status: protocol::PresenceStatus::Idle,
                version: 7,
                platforms: vec![protocol::Platform::Android],
                devices: vec![protocol::Device {
                    platform: protocol::Platform::Android,
                    form: Some(protocol::DeviceForm::Phone),
                }],
            }),
        }],
        blocked: Vec::new(),
        read_states: Vec::new(),
    };
    let json = round_trip(&ready);
    assert_eq!(json["d"]["dms"][0]["id"], "300");
    assert_eq!(json["d"]["relationships"][0]["kind"], "friend");
    assert_eq!(json["d"]["relationships"][0]["presence"]["status"], "idle");
    let mut old = json.clone();
    old["d"].as_object_mut().unwrap().remove("relationships");
    let decoded: GatewayMessage = serde_json::from_value(old).unwrap();
    assert!(
        matches!(decoded, GatewayMessage::Ready { relationships, .. } if relationships.is_empty())
    );

    // An older server sends no `dms`: decoded as none.
    let mut old = json.clone();
    old["d"].as_object_mut().unwrap().remove("dms");
    let decoded: GatewayMessage = serde_json::from_value(old).unwrap();
    let GatewayMessage::Ready { dms, guilds, .. } = decoded else {
        panic!("expected ready");
    };
    assert!(dms.is_empty());
    assert_eq!(guilds, [guild()]);

    // An older client's `ready` has no `dms` field and skips it (unknown
    // fields are ignored); DM events reach it as `Event::Unknown`, see
    // `unknown_event_types_decode_as_unknown` in `wire.rs`.
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct OldReady {
        session_id: String,
        guilds: Vec<Guild>,
    }
    let old: OldReady = serde_json::from_value(json["d"].clone()).unwrap();
    assert_eq!(old.session_id, "s-1");
    assert_eq!(old.guilds, [guild()]);
}

#[test]
fn moderation_bodies() {
    use protocol::rest::{CreateBanRequest, GuildBan, ModerateVoiceStateRequest};

    // Absent flags stay as they are, so they are left out.
    let mute = ModerateVoiceStateRequest {
        server_mute: Some(true),
        server_deaf: None,
    };
    assert_eq!(round_trip(&mute), json!({"serverMute": true}));
    let empty: ModerateVoiceStateRequest = serde_json::from_value(json!({})).unwrap();
    assert_eq!(empty, ModerateVoiceStateRequest::default());

    assert_eq!(round_trip(&CreateBanRequest::default()), json!({}));
    let ban = GuildBan {
        user: user(),
        reason: Some("spam".into()),
        banned_by: Some(GUILD),
        created_at: Timestamp::from_unix_millis(1_761_000_000_000),
    };
    let json = round_trip(&ban);
    assert_eq!(json["bannedBy"], "100");
    assert_eq!(json["createdAt"], 1_761_000_000_000_u64);
    // Who banned is optional (the account may be gone).
    let mut gone = json.clone();
    gone.as_object_mut().unwrap().remove("bannedBy");
    let decoded: GuildBan = serde_json::from_value(gone).unwrap();
    assert_eq!(decoded.banned_by, None);
}
