//! Core data models shared by REST and the gateway (architecture.md §9.1).

mod channel;
mod component;
mod dm;
mod guild;
mod interaction;
mod invite;
mod member;
mod message;
mod overwrite;
mod presence;
mod relationship;
mod role;
mod user;
mod voice;

pub use channel::{AudioProfile, Channel, ChannelKind, VoiceSettings};
pub use component::{ActionRow, Button, ButtonStyle, Component};
pub use dm::DmChannel;
pub use guild::{Guild, GuildAccess, GuildCategory};
pub use interaction::{Interaction, InteractionKind};
pub use invite::Invite;
pub use member::Member;
pub use message::{Attachment, Message, MessageReply};
pub use overwrite::{OverwriteTarget, PermissionOverwrite};
pub use presence::{Device, DeviceForm, Platform, PresenceStatus, StatusSetting};
pub use relationship::{Relationship, RelationshipKind};
pub use role::Role;
pub use user::User;
pub use voice::{CAMERA_IDENTITY_SUFFIX, CameraSource, VoiceDevice, VoiceState};
