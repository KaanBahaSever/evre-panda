//! What the bot knows about its guild, kept current from gateway events.
//!
//! Filled by the `guildCreate` snapshots that follow `ready` and updated by
//! every dispatch before your code sees it, so in an event handler the
//! cache already reflects that event.

use std::collections::HashMap;
use std::sync::RwLock;

use protocol::rest::{GuildMember, GuildSnapshot};
use protocol::{Channel, Event, Guild, Permissions, Snowflake, User, VoiceState};

#[derive(Debug, Default)]
struct State {
    me: Option<User>,
    guilds: HashMap<Snowflake, GuildSnapshot>,
    /// Channel → guild.
    channels: HashMap<Snowflake, Snowflake>,
}

/// The bot's view of its guilds. Reads return clones; cheap enough for a
/// bot's guild sizes.
#[derive(Debug, Default)]
pub struct Cache {
    state: RwLock<State>,
}

/// The name to show for a member: nickname, else display name, else
/// username.
#[must_use]
pub fn shown_name(member: &GuildMember) -> &str {
    member
        .member
        .nick
        .as_deref()
        .or(member.user.display_name.as_deref())
        .unwrap_or(&member.user.username)
}

impl Cache {
    fn read(&self) -> std::sync::RwLockReadGuard<'_, State> {
        self.state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, State> {
        self.state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The bot's own account, once `ready` arrived.
    #[must_use]
    pub fn me(&self) -> Option<User> {
        self.read().me.clone()
    }

    /// IDs of the guilds the bot is in.
    #[must_use]
    pub fn guild_ids(&self) -> Vec<Snowflake> {
        let mut ids: Vec<_> = self.read().guilds.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    /// The whole snapshot of one guild.
    #[must_use]
    pub fn guild(&self, guild_id: Snowflake) -> Option<GuildSnapshot> {
        self.read().guilds.get(&guild_id).cloned()
    }

    /// A guild's settings.
    #[must_use]
    pub fn guild_info(&self, guild_id: Snowflake) -> Option<Guild> {
        self.read()
            .guilds
            .get(&guild_id)
            .map(|snapshot| snapshot.guild.clone())
    }

    /// A channel the bot can see.
    #[must_use]
    pub fn channel(&self, channel_id: Snowflake) -> Option<Channel> {
        let state = self.read();
        let guild_id = state.channels.get(&channel_id)?;
        state
            .guilds
            .get(guild_id)?
            .channels
            .iter()
            .find(|channel| channel.id == channel_id)
            .cloned()
    }

    /// The guild a channel belongs to.
    #[must_use]
    pub fn guild_of_channel(&self, channel_id: Snowflake) -> Option<Snowflake> {
        self.read().channels.get(&channel_id).copied()
    }

    /// A member of a guild with its user.
    #[must_use]
    pub fn member(&self, guild_id: Snowflake, user_id: Snowflake) -> Option<GuildMember> {
        self.read()
            .guilds
            .get(&guild_id)?
            .members
            .iter()
            .find(|member| member.user.id == user_id)
            .cloned()
    }

    /// A user known from any guild's member list.
    #[must_use]
    pub fn user(&self, user_id: Snowflake) -> Option<User> {
        let state = self.read();
        if let Some(me) = state.me.as_ref().filter(|me| me.id == user_id) {
            return Some(me.clone());
        }
        state.guilds.values().find_map(|snapshot| {
            snapshot
                .members
                .iter()
                .find(|member| member.user.id == user_id)
                .map(|member| member.user.clone())
        })
    }

    /// How a member is shown in `guild_id` (nickname, display name or
    /// username); the ID if unknown.
    #[must_use]
    pub fn display_name(&self, guild_id: Snowflake, user_id: Snowflake) -> String {
        self.member(guild_id, user_id).map_or_else(
            || user_id.to_string(),
            |member| shown_name(&member).to_owned(),
        )
    }

    /// A member's voice state, if they are in voice. A member in voice on
    /// a desktop and a phone at once (ADR-076) has two; this is the
    /// desktop's.
    #[must_use]
    pub fn voice_state(&self, guild_id: Snowflake, user_id: Snowflake) -> Option<VoiceState> {
        self.read()
            .guilds
            .get(&guild_id)?
            .voice_states
            .iter()
            .filter(|state| state.user_id == user_id)
            .min_by_key(|state| state.device)
            .cloned()
    }

    /// The voice channel a member is in.
    #[must_use]
    pub fn voice_channel_of(&self, guild_id: Snowflake, user_id: Snowflake) -> Option<Snowflake> {
        self.voice_state(guild_id, user_id)
            .and_then(|state| state.channel_id)
    }

    /// Who is in a voice channel (user IDs), the bot included if it is.
    #[must_use]
    pub fn voice_members(&self, channel_id: Snowflake) -> Vec<Snowflake> {
        let state = self.read();
        let Some(snapshot) = state
            .channels
            .get(&channel_id)
            .and_then(|guild_id| state.guilds.get(guild_id))
        else {
            return Vec::new();
        };
        snapshot
            .voice_states
            .iter()
            .filter(|voice| voice.channel_id == Some(channel_id))
            .map(|voice| voice.user_id)
            .collect()
    }

    /// The bot's own permissions in a channel.
    #[must_use]
    pub fn permissions_in(&self, channel_id: Snowflake) -> Option<Permissions> {
        let state = self.read();
        let guild_id = state.channels.get(&channel_id)?;
        state
            .guilds
            .get(guild_id)?
            .channel_permissions
            .iter()
            .find(|entry| entry.channel_id == channel_id)
            .map(|entry| entry.permissions)
    }

    /// A fresh session: forget everything but the user.
    pub(crate) fn reset(&self, me: User) {
        let mut state = self.write();
        state.me = Some(me);
        state.guilds.clear();
        state.channels.clear();
    }

    /// Applies one dispatch.
    pub(crate) fn apply(&self, event: &Event) {
        let mut state = self.write();
        let State {
            me,
            guilds,
            channels,
        } = &mut *state;
        match event {
            Event::GuildCreate(snapshot) => {
                let guild_id = snapshot.guild.id;
                channels.retain(|_, owner| *owner != guild_id);
                for channel in &snapshot.channels {
                    channels.insert(channel.id, guild_id);
                }
                guilds.insert(guild_id, snapshot.clone());
            }
            Event::GuildUpdate(guild) => {
                if let Some(snapshot) = guilds.get_mut(&guild.id) {
                    snapshot.guild = guild.clone();
                }
            }
            Event::ChannelCreate(channel) | Event::ChannelUpdate(channel) => {
                if let Some(snapshot) = guilds.get_mut(&channel.guild_id) {
                    match snapshot.channels.iter_mut().find(|c| c.id == channel.id) {
                        Some(existing) => *existing = channel.clone(),
                        None => snapshot.channels.push(channel.clone()),
                    }
                    snapshot.channels.sort_by_key(|c| (c.position, c.id));
                    channels.insert(channel.id, channel.guild_id);
                }
            }
            Event::ChannelDelete(deleted) => {
                if let Some(snapshot) = guilds.get_mut(&deleted.guild_id) {
                    snapshot.channels.retain(|c| c.id != deleted.id);
                    snapshot
                        .voice_states
                        .retain(|voice| voice.channel_id != Some(deleted.id));
                }
                channels.remove(&deleted.id);
            }
            Event::GuildMemberAdd(added) => {
                if let Some(snapshot) = guilds.get_mut(&added.member.guild_id) {
                    let entry = GuildMember {
                        member: added.member.clone(),
                        user: added.user.clone(),
                    };
                    match snapshot
                        .members
                        .iter_mut()
                        .find(|m| m.user.id == added.user.id)
                    {
                        Some(existing) => *existing = entry,
                        None => snapshot.members.push(entry),
                    }
                }
            }
            Event::GuildMemberUpdate(member) => {
                if let Some(snapshot) = guilds.get_mut(&member.guild_id)
                    && let Some(existing) = snapshot
                        .members
                        .iter_mut()
                        .find(|m| m.user.id == member.user_id)
                {
                    existing.member = member.clone();
                }
            }
            Event::GuildMemberRemove(removed) => {
                if me.as_ref().is_some_and(|me| me.id == removed.user_id) {
                    guilds.remove(&removed.guild_id);
                    channels.retain(|_, owner| *owner != removed.guild_id);
                } else if let Some(snapshot) = guilds.get_mut(&removed.guild_id) {
                    snapshot.members.retain(|m| m.user.id != removed.user_id);
                    snapshot
                        .voice_states
                        .retain(|voice| voice.user_id != removed.user_id);
                }
            }
            Event::VoiceStateUpdate(voice) => {
                if let Some(snapshot) = guilds.get_mut(&voice.guild_id) {
                    snapshot.voice_states.retain(|existing| {
                        existing.user_id != voice.user_id || existing.device != voice.device
                    });
                    if voice.channel_id.is_some() {
                        snapshot.voice_states.push(voice.clone());
                    }
                }
            }
            Event::UserUpdate(user) => {
                if let Some(me) = me.as_mut().filter(|me| me.id == user.id) {
                    *me = user.clone();
                }
                for snapshot in guilds.values_mut() {
                    for member in &mut snapshot.members {
                        if member.user.id == user.id {
                            member.user = user.clone();
                        }
                    }
                }
            }
            Event::GuildRoleCreate(role) | Event::GuildRoleUpdate(role) => {
                if let Some(snapshot) = guilds.get_mut(&role.guild_id) {
                    match snapshot.roles.iter_mut().find(|r| r.id == role.id) {
                        Some(existing) => *existing = role.clone(),
                        None => snapshot.roles.push(role.clone()),
                    }
                }
            }
            Event::GuildRoleDelete(deleted) => {
                if let Some(snapshot) = guilds.get_mut(&deleted.guild_id) {
                    snapshot.roles.retain(|r| r.id != deleted.role_id);
                    for member in &mut snapshot.members {
                        member.member.role_ids.retain(|id| *id != deleted.role_id);
                    }
                }
            }
            Event::PresenceUpdate(update) => {
                for snapshot in guilds.values_mut() {
                    if !snapshot.members.iter().any(|m| m.user.id == update.user_id) {
                        continue;
                    }
                    match snapshot
                        .presences
                        .iter_mut()
                        .find(|p| p.user_id == update.user_id)
                    {
                        Some(existing) if existing.version > update.version => {}
                        Some(existing) => *existing = update.clone(),
                        None => snapshot.presences.push(update.clone()),
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use protocol::gateway::{ChannelDelete, GuildMemberAdd, GuildMemberRemove};
    use protocol::rest::ChannelPermissions;
    use protocol::{ChannelKind, Member, PlanId, Timestamp};

    use super::*;

    pub(crate) fn user(id: u64, name: &str) -> User {
        User {
            id: Snowflake::new(id),
            username: name.into(),
            display_name: None,
            avatar: None,
            banner: None,
            bio: None,
            bot: false,
            deleted: false,
        }
    }

    fn member(guild: u64, user: &User) -> GuildMember {
        GuildMember {
            member: Member {
                user_id: user.id,
                guild_id: Snowflake::new(guild),
                nick: None,
                role_ids: Vec::new(),
                joined_at: Timestamp::from_unix_millis(1),
            },
            user: user.clone(),
        }
    }

    fn channel(guild: u64, id: u64, kind: ChannelKind) -> Channel {
        Channel {
            id: Snowflake::new(id),
            guild_id: Snowflake::new(guild),
            kind,
            name: format!("c{id}"),
            parent_id: None,
            position: i32::try_from(id).unwrap(),
            topic: None,
            voice: None,
            permission_overwrites: Vec::new(),
        }
    }

    pub(crate) fn snapshot(guild: u64, members: &[&User]) -> GuildSnapshot {
        GuildSnapshot {
            guild: Guild {
                id: Snowflake::new(guild),
                name: "Bizim Sunucu".into(),
                owner_id: members[0].id,
                plan: PlanId::Internal,
                icon: None,
                banner: None,
                access: protocol::GuildAccess::InviteOnly,
                description: None,
                category: None,
                locale: None,
            },
            channels: vec![
                channel(guild, 10, ChannelKind::Text),
                channel(guild, 11, ChannelKind::Voice),
            ],
            roles: Vec::new(),
            members: members.iter().map(|user| member(guild, user)).collect(),
            voice_states: Vec::new(),
            permissions: Permissions::all(),
            channel_permissions: vec![ChannelPermissions {
                channel_id: Snowflake::new(10),
                permissions: Permissions::SEND_MESSAGES,
            }],
            presences: Vec::new(),
            presence_version: 0,
            read_states: Vec::new(),
            open_reports: 0,
            pending_join_requests: 0,
        }
    }

    fn voice(guild: u64, user: Snowflake, channel: Option<u64>) -> VoiceState {
        VoiceState {
            guild_id: Snowflake::new(guild),
            user_id: user,
            channel_id: channel.map(Snowflake::new),
            self_mute: false,
            self_deaf: false,
            server_mute: false,
            server_deaf: false,
            streaming: false,
            streaming_since: None,
            watching: Vec::new(),
            away: false,
            moved_by: None,
            connection_id: None,
            device: protocol::VoiceDevice::Desktop,
            camera: false,
            camera_source: None,
        }
    }

    #[test]
    fn follows_members_channels_and_voice() {
        let bot = User {
            bot: true,
            deleted: false,
            ..user(1, "panda")
        };
        let kaan = User {
            display_name: Some("Kaan".into()),
            ..user(2, "kaan")
        };
        let cache = Cache::default();
        cache.reset(bot.clone());
        cache.apply(&Event::GuildCreate(snapshot(100, &[&kaan, &bot])));
        let guild = Snowflake::new(100);
        assert_eq!(cache.guild_ids(), [guild]);
        assert_eq!(cache.guild_of_channel(Snowflake::new(11)), Some(guild));
        assert_eq!(cache.display_name(guild, kaan.id), "Kaan");
        assert_eq!(cache.user(bot.id).map(|u| u.bot), Some(true));
        assert_eq!(
            cache.permissions_in(Snowflake::new(10)),
            Some(Permissions::SEND_MESSAGES)
        );

        // Voice: join, move, leave.
        cache.apply(&Event::VoiceStateUpdate(voice(100, kaan.id, Some(11))));
        assert_eq!(
            cache.voice_channel_of(guild, kaan.id),
            Some(Snowflake::new(11))
        );
        assert_eq!(cache.voice_members(Snowflake::new(11)), [kaan.id]);
        cache.apply(&Event::VoiceStateUpdate(voice(100, kaan.id, None)));
        assert_eq!(cache.voice_channel_of(guild, kaan.id), None);
        assert!(cache.voice_members(Snowflake::new(11)).is_empty());

        // New member, then removed.
        let ece = user(3, "ece");
        cache.apply(&Event::GuildMemberAdd(GuildMemberAdd {
            member: member(100, &ece).member,
            user: ece.clone(),
        }));
        assert_eq!(cache.display_name(guild, ece.id), "ece");
        cache.apply(&Event::VoiceStateUpdate(voice(100, ece.id, Some(11))));
        cache.apply(&Event::GuildMemberRemove(GuildMemberRemove {
            guild_id: guild,
            user_id: ece.id,
        }));
        assert!(cache.member(guild, ece.id).is_none());
        assert!(cache.voice_members(Snowflake::new(11)).is_empty());

        // Channels.
        let mut renamed = channel(100, 10, ChannelKind::Text);
        renamed.name = "sohbet".into();
        cache.apply(&Event::ChannelUpdate(renamed));
        assert_eq!(cache.channel(Snowflake::new(10)).unwrap().name, "sohbet");
        cache.apply(&Event::ChannelDelete(ChannelDelete {
            id: Snowflake::new(10),
            guild_id: guild,
        }));
        assert!(cache.channel(Snowflake::new(10)).is_none());

        // The bot itself removed: the guild is gone.
        cache.apply(&Event::GuildMemberRemove(GuildMemberRemove {
            guild_id: guild,
            user_id: bot.id,
        }));
        assert!(cache.guild_ids().is_empty());
        assert!(cache.guild_of_channel(Snowflake::new(11)).is_none());
    }

    #[test]
    fn user_updates_reach_members_and_me() {
        let bot = user(1, "panda");
        let cache = Cache::default();
        cache.reset(bot.clone());
        cache.apply(&Event::GuildCreate(snapshot(100, &[&bot])));
        let renamed = User {
            display_name: Some("Panda 🐼".into()),
            ..bot.clone()
        };
        cache.apply(&Event::UserUpdate(renamed.clone()));
        assert_eq!(cache.me(), Some(renamed.clone()));
        assert_eq!(cache.display_name(Snowflake::new(100), bot.id), "Panda 🐼");
    }
}
