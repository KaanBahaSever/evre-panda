use serde::{Deserialize, Serialize};

/// Online status of a user, as others see it (architecture.md §8.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PresenceStatus {
    /// Online and active.
    Online,
    /// Away from keyboard.
    Idle,
    /// Online, but notifications are muted.
    DoNotDisturb,
    /// Offline or invisible.
    Offline,
}

/// The status a user chose for themselves. Stored per user, so it applies
/// to all of their devices and survives restarts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatusSetting {
    /// Automatic: online while any device is active, idle when all are idle.
    #[default]
    Online,
    /// Always shown as idle.
    Idle,
    /// Shown as do-not-disturb; notifications are muted.
    DoNotDisturb,
    /// Shown as offline to everyone else.
    Invisible,
}

impl StatusSetting {
    /// What others see for a user with this setting and at least one
    /// connected session. `all_idle`: every connected session reported idle.
    #[must_use]
    pub const fn shown(self, all_idle: bool) -> PresenceStatus {
        match self {
            Self::Online if all_idle => PresenceStatus::Idle,
            Self::Online => PresenceStatus::Online,
            Self::Idle => PresenceStatus::Idle,
            Self::DoNotDisturb => PresenceStatus::DoNotDisturb,
            Self::Invisible => PresenceStatus::Offline,
        }
    }
}

/// The kind of device a client runs on, reported at identify and shown to
/// others next to an online user ("Windows'tan çevrimiçi", §8.4). Ordered
/// desktop first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    /// Windows desktop.
    Windows,
    /// macOS desktop.
    MacOs,
    /// Linux desktop.
    Linux,
    /// iPhone or iPad.
    Ios,
    /// Android phone or tablet.
    Android,
    /// A platform introduced after this build. Decoded only, never sent;
    /// clients should ignore it.
    #[serde(other)]
    Unknown,
}

impl Platform {
    /// The platform of a `std::env::consts::OS` value (`"windows"`,
    /// `"macos"`, `"linux"`, `"ios"`, `"android"`); `None` for others.
    #[must_use]
    pub fn from_os(os: &str) -> Option<Self> {
        match os {
            "windows" => Some(Self::Windows),
            "macos" => Some(Self::MacOs),
            "linux" => Some(Self::Linux),
            "ios" => Some(Self::Ios),
            "android" => Some(Self::Android),
            _ => None,
        }
    }

    /// A phone or tablet.
    #[must_use]
    pub const fn is_mobile(self) -> bool {
        matches!(self, Self::Ios | Self::Android)
    }
}

/// What shape of device a client runs on, reported at identify next to the
/// [`Platform`] (§8.4): together they say "Windows laptop", "MacBook",
/// "iPhone". Ordered desktop first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceForm {
    /// A desktop computer: tower, all-in-one, mini PC (iMac, Mac mini).
    Desktop,
    /// A laptop (MacBook).
    Laptop,
    /// A tablet (iPad).
    Tablet,
    /// A phone.
    Phone,
    /// A form introduced after this build. Decoded only, never sent;
    /// treated as not said.
    #[serde(other)]
    Unknown,
}

/// One kind of device an online user is on (§8.4): its platform and, when
/// its client said, its form. Presences list each kind once, in this
/// type's order: computers first (desktops, then laptops), then tablets and
/// phones; within a form by [`Platform`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// The device's platform.
    pub platform: Platform,
    /// The device's form; left out by clients that cannot tell or are
    /// older than forms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<DeviceForm>,
}

impl Device {
    /// The form to show: the one the client named, else what devices of the
    /// platform usually are (a desktop computer, or a phone on iOS and
    /// Android).
    #[must_use]
    pub const fn shown_form(self) -> DeviceForm {
        match self.form {
            Some(
                form @ (DeviceForm::Desktop
                | DeviceForm::Laptop
                | DeviceForm::Tablet
                | DeviceForm::Phone),
            ) => form,
            Some(DeviceForm::Unknown) | None if self.platform.is_mobile() => DeviceForm::Phone,
            Some(DeviceForm::Unknown) | None => DeviceForm::Desktop,
        }
    }
}

impl Ord for Device {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.shown_form(), self.platform, self.form).cmp(&(
            other.shown_form(),
            other.platform,
            other.form,
        ))
    }
}

impl PartialOrd for Device {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platforms_on_the_wire() {
        for (platform, wire) in [
            (Platform::Windows, "windows"),
            (Platform::MacOs, "macos"),
            (Platform::Linux, "linux"),
            (Platform::Ios, "ios"),
            (Platform::Android, "android"),
        ] {
            let json = serde_json::to_value(platform).unwrap();
            assert_eq!(json, wire);
            assert_eq!(Platform::from_os(wire), Some(platform));
            assert_eq!(serde_json::from_value::<Platform>(json).unwrap(), platform);
        }
        assert_eq!(
            serde_json::from_value::<Platform>("web".into()).unwrap(),
            Platform::Unknown
        );
        assert_eq!(Platform::from_os("freebsd"), None);
        assert!(Platform::Ios.is_mobile() && !Platform::MacOs.is_mobile());
    }

    #[test]
    fn device_forms_on_the_wire() {
        for (form, wire) in [
            (DeviceForm::Desktop, "desktop"),
            (DeviceForm::Laptop, "laptop"),
            (DeviceForm::Tablet, "tablet"),
            (DeviceForm::Phone, "phone"),
        ] {
            let json = serde_json::to_value(form).unwrap();
            assert_eq!(json, wire);
            assert_eq!(serde_json::from_value::<DeviceForm>(json).unwrap(), form);
        }
        assert_eq!(
            serde_json::from_value::<DeviceForm>("watch".into()).unwrap(),
            DeviceForm::Unknown
        );
        let macbook = Device {
            platform: Platform::MacOs,
            form: Some(DeviceForm::Laptop),
        };
        assert_eq!(
            serde_json::to_value(macbook).unwrap(),
            serde_json::json!({"platform": "macos", "form": "laptop"})
        );
        // Without a form (older clients) the field is left out.
        let old = Device {
            platform: Platform::Windows,
            form: None,
        };
        assert_eq!(
            serde_json::to_value(old).unwrap(),
            serde_json::json!({"platform": "windows"})
        );
    }

    #[test]
    fn devices_without_a_form_show_the_usual_one() {
        let device = |platform, form| Device { platform, form };
        assert_eq!(
            device(Platform::Windows, None).shown_form(),
            DeviceForm::Desktop
        );
        assert_eq!(
            device(Platform::Android, Some(DeviceForm::Unknown)).shown_form(),
            DeviceForm::Phone
        );
        assert_eq!(
            device(Platform::Ios, Some(DeviceForm::Tablet)).shown_form(),
            DeviceForm::Tablet
        );
    }

    #[test]
    fn devices_sort_computers_first() {
        let device = |platform, form| Device { platform, form };
        let mut devices = vec![
            device(Platform::Ios, Some(DeviceForm::Phone)),
            device(Platform::Android, Some(DeviceForm::Tablet)),
            device(Platform::MacOs, Some(DeviceForm::Laptop)),
            device(Platform::Android, None),
            device(Platform::Linux, Some(DeviceForm::Desktop)),
            device(Platform::Windows, None),
            device(Platform::Windows, Some(DeviceForm::Desktop)),
        ];
        devices.sort();
        assert_eq!(
            devices,
            [
                device(Platform::Windows, None),
                device(Platform::Windows, Some(DeviceForm::Desktop)),
                device(Platform::Linux, Some(DeviceForm::Desktop)),
                device(Platform::MacOs, Some(DeviceForm::Laptop)),
                device(Platform::Android, Some(DeviceForm::Tablet)),
                device(Platform::Ios, Some(DeviceForm::Phone)),
                device(Platform::Android, None),
            ]
        );
    }

    #[test]
    fn shown_status() {
        assert_eq!(StatusSetting::Online.shown(false), PresenceStatus::Online);
        assert_eq!(StatusSetting::Online.shown(true), PresenceStatus::Idle);
        assert_eq!(
            StatusSetting::DoNotDisturb.shown(true),
            PresenceStatus::DoNotDisturb
        );
        assert_eq!(
            StatusSetting::Invisible.shown(false),
            PresenceStatus::Offline
        );
    }
}
