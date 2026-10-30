//! Permission bits (architecture.md §10).
//!
//! The bits live here so clients and bots can read them; the computation
//! engine (roles + channel overwrites) lives in `server_core::permissions`.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::serde_util::u64_string;

bitflags::bitflags! {
    /// A set of permissions, stored as `u64` bit flags.
    ///
    /// Bit positions are part of the wire contract: never renumber or reuse
    /// them. On the wire the set is a decimal string (`"1027"`). Bits unknown
    /// to this build are preserved when decoding, so a client never strips
    /// permissions a newer server introduced.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Permissions: u64 {
        /// See the channel. Without it a member has no permission in that
        /// channel at all.
        const VIEW_CHANNEL = 1 << 0;
        /// Send messages in text channels.
        const SEND_MESSAGES = 1 << 1;
        /// Read messages sent before the member opened the channel.
        const READ_MESSAGE_HISTORY = 1 << 2;
        /// Upload files. Requires `SEND_MESSAGES`.
        const ATTACH_FILES = 1 << 3;
        /// Links show previews/embeds. Requires `SEND_MESSAGES`.
        const EMBED_LINKS = 1 << 4;
        /// Add new reactions to messages.
        const ADD_REACTIONS = 1 << 5;
        /// Use `@everyone` and `@here`. Requires `SEND_MESSAGES`.
        const MENTION_EVERYONE = 1 << 6;
        /// Delete or pin other members' messages.
        const MANAGE_MESSAGES = 1 << 7;
        /// Join a voice channel (always an explicit user action, §6.6).
        const CONNECT = 1 << 8;
        /// Speak in a voice channel.
        const SPEAK = 1 << 9;
        /// Share a screen or application in a voice channel.
        const STREAM = 1 << 10;
        /// Use voice activation; without it the member must use push-to-talk.
        const USE_VOICE_ACTIVITY = 1 << 11;
        /// Be heard over others (others are ducked while speaking).
        const PRIORITY_SPEAKER = 1 << 12;
        /// Server-mute other members in voice.
        const MUTE_MEMBERS = 1 << 13;
        /// Server-deafen other members in voice.
        const DEAFEN_MEMBERS = 1 << 14;
        /// Move members who are already in voice to another channel.
        const MOVE_MEMBERS = 1 << 15;
        /// Create, edit and delete channels.
        const MANAGE_CHANNELS = 1 << 16;
        /// Create and edit roles below one's own; edit channel overwrites.
        const MANAGE_ROLES = 1 << 17;
        /// Edit guild settings.
        const MANAGE_GUILD = 1 << 18;
        /// Remove members from the guild.
        const KICK_MEMBERS = 1 << 19;
        /// Ban members from the guild.
        const BAN_MEMBERS = 1 << 20;
        /// Create invites.
        const CREATE_INVITE = 1 << 21;
        /// Manage webhooks and customer-run bots.
        const MANAGE_WEBHOOKS_AND_BOTS = 1 << 22;
        /// View the guild audit log (within the plan's retention).
        const VIEW_AUDIT_LOG = 1 << 23;
        /// Every permission everywhere; bypasses channel overwrites.
        const ADMINISTRATOR = 1 << 24;
    }
}

impl Permissions {
    /// Removed implicitly when `SEND_MESSAGES` is missing.
    pub const REQUIRES_SEND_MESSAGES: Self = Self::ATTACH_FILES
        .union(Self::EMBED_LINKS)
        .union(Self::MENTION_EVERYONE);

    /// Removed implicitly in a voice channel when `CONNECT` is missing.
    pub const REQUIRES_CONNECT: Self = Self::SPEAK
        .union(Self::STREAM)
        .union(Self::USE_VOICE_ACTIVITY)
        .union(Self::PRIORITY_SPEAKER)
        .union(Self::MUTE_MEMBERS)
        .union(Self::DEAFEN_MEMBERS)
        .union(Self::MOVE_MEMBERS);

    /// Guild-wide permissions. Channel overwrites cannot grant or revoke them.
    pub const GUILD_ONLY: Self = Self::MANAGE_GUILD
        .union(Self::KICK_MEMBERS)
        .union(Self::BAN_MEMBERS)
        .union(Self::MANAGE_WEBHOOKS_AND_BOTS)
        .union(Self::VIEW_AUDIT_LOG)
        .union(Self::ADMINISTRATOR);

    /// Permissions that channel overwrites may grant or revoke.
    pub const CHANNEL_SCOPED: Self = Self::all().difference(Self::GUILD_ONLY);
}

impl Default for Permissions {
    fn default() -> Self {
        Self::empty()
    }
}

impl Serialize for Permissions {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        u64_string::serialize(self.bits(), serializer)
    }
}

impl<'de> Deserialize<'de> for Permissions {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        u64_string::deserialize(deserializer).map(Self::from_bits_retain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_positions_are_stable() {
        let expected = [
            (Permissions::VIEW_CHANNEL, 0),
            (Permissions::SEND_MESSAGES, 1),
            (Permissions::READ_MESSAGE_HISTORY, 2),
            (Permissions::ATTACH_FILES, 3),
            (Permissions::EMBED_LINKS, 4),
            (Permissions::ADD_REACTIONS, 5),
            (Permissions::MENTION_EVERYONE, 6),
            (Permissions::MANAGE_MESSAGES, 7),
            (Permissions::CONNECT, 8),
            (Permissions::SPEAK, 9),
            (Permissions::STREAM, 10),
            (Permissions::USE_VOICE_ACTIVITY, 11),
            (Permissions::PRIORITY_SPEAKER, 12),
            (Permissions::MUTE_MEMBERS, 13),
            (Permissions::DEAFEN_MEMBERS, 14),
            (Permissions::MOVE_MEMBERS, 15),
            (Permissions::MANAGE_CHANNELS, 16),
            (Permissions::MANAGE_ROLES, 17),
            (Permissions::MANAGE_GUILD, 18),
            (Permissions::KICK_MEMBERS, 19),
            (Permissions::BAN_MEMBERS, 20),
            (Permissions::CREATE_INVITE, 21),
            (Permissions::MANAGE_WEBHOOKS_AND_BOTS, 22),
            (Permissions::VIEW_AUDIT_LOG, 23),
            (Permissions::ADMINISTRATOR, 24),
        ];
        for (flag, bit) in expected {
            assert_eq!(flag.bits(), 1 << bit, "{flag:?}");
        }
        assert_eq!(Permissions::all().bits(), (1 << 25) - 1);
    }

    #[test]
    fn serde_uses_decimal_strings() {
        let perms = Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::CONNECT;
        let json = serde_json::to_string(&perms).unwrap();
        assert_eq!(json, "\"259\"");
        assert_eq!(serde_json::from_str::<Permissions>(&json).unwrap(), perms);
        assert_eq!(
            serde_json::to_string(&Permissions::empty()).unwrap(),
            "\"0\""
        );
    }

    #[test]
    fn serde_rejects_numbers() {
        assert!(serde_json::from_str::<Permissions>("259").is_err());
        assert!(serde_json::from_str::<Permissions>("\"x\"").is_err());
    }

    #[test]
    fn unknown_bits_survive_a_round_trip() {
        let json = format!("\"{}\"", (1_u64 << 63) | 1);
        let perms: Permissions = serde_json::from_str(&json).unwrap();
        assert!(perms.contains(Permissions::VIEW_CHANNEL));
        assert_eq!(perms.bits(), (1 << 63) | 1);
        assert_eq!(serde_json::to_string(&perms).unwrap(), json);
    }

    #[test]
    fn masks_partition_the_known_bits() {
        assert!(
            Permissions::CHANNEL_SCOPED
                .intersection(Permissions::GUILD_ONLY)
                .is_empty()
        );
        assert_eq!(
            Permissions::CHANNEL_SCOPED.union(Permissions::GUILD_ONLY),
            Permissions::all()
        );
        assert!(Permissions::CHANNEL_SCOPED.contains(Permissions::REQUIRES_CONNECT));
        assert!(Permissions::CHANNEL_SCOPED.contains(Permissions::REQUIRES_SEND_MESSAGES));
        assert!(!Permissions::CHANNEL_SCOPED.contains(Permissions::ADMINISTRATOR));
        assert_eq!(Permissions::default(), Permissions::empty());
    }
}
