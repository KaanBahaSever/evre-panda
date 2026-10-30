//! The languages Evre's server writes human texts in (architecture.md
//! §11.7, §8.1; ADR-083): verification and security e-mails and push
//! titles and bodies. Error codes and other machine values are never
//! translated.
//!
//! A client says which language it wants with an IETF language tag:
//! `ClientInfo.locale` in the gateway's identify, or the `Accept-Language`
//! header on the auth routes. [`Locale::from_tag`] and
//! [`Locale::from_accept_language`] map that to one of the five, else
//! [`Locale::FALLBACK`] (English). An account that never said keeps
//! [`Locale::LEGACY`] (Turkish): the apps that do not send one are Turkish.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// A language the server writes texts in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    /// Turkish ([`Locale::LEGACY`], the default).
    #[default]
    Tr,
    /// English.
    En,
    /// German.
    De,
    /// French.
    Fr,
    /// Spanish.
    Es,
}

impl Locale {
    /// Every locale, Turkish first.
    pub const ALL: [Self; 5] = [Self::Tr, Self::En, Self::De, Self::Fr, Self::Es];
    /// For a language the client asked for that Evre does not have.
    pub const FALLBACK: Self = Self::En;
    /// For an account that never said (apps without locale support are
    /// Turkish).
    pub const LEGACY: Self = Self::Tr;

    /// The two-letter code, as stored and sent (`"tr"`, `"en"`, …).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Tr => "tr",
            Self::En => "en",
            Self::De => "de",
            Self::Fr => "fr",
            Self::Es => "es",
        }
    }

    /// One of the five by its primary language subtag (`"de-AT"`,
    /// `"pt_BR"`, `"FR"`: case-insensitive, `-` or `_`), or `None` when the
    /// language is not one Evre has (or the tag is empty or malformed).
    #[must_use]
    pub fn supported(tag: &str) -> Option<Self> {
        let primary = tag.trim().split(['-', '_']).next().unwrap_or_default();
        Self::ALL
            .into_iter()
            .find(|locale| primary.eq_ignore_ascii_case(locale.code()))
    }

    /// The locale for a tag the client sent: one of the five, else
    /// [`Self::FALLBACK`]. `None` only when nothing usable was sent (an
    /// empty or overlong value), so the stored choice stays.
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        let tag = tag.trim();
        if tag.is_empty() || tag.len() > MAX_TAG {
            return None;
        }
        Some(Self::supported(tag).unwrap_or(Self::FALLBACK))
    }

    /// The locale an `Accept-Language` header asks for: the supported
    /// language with the highest weight (the first among equals; `q=0`
    /// excluded), else [`Self::FALLBACK`] when it lists languages but none
    /// of ours. `None` when the header is empty, only `*`, or not a header
    /// we can read, so the stored choice stays.
    #[must_use]
    pub fn from_accept_language(header: &str) -> Option<Self> {
        if header.len() > MAX_HEADER {
            return None;
        }
        let mut best: Option<(u16, Self)> = None;
        let mut listed = false;
        for item in header.split(',').take(MAX_ITEMS) {
            let mut parts = item.split(';');
            let tag = parts.next().unwrap_or_default().trim();
            if tag.is_empty() || tag == "*" {
                continue;
            }
            let weight = parts
                .find_map(|param| {
                    let (key, value) = param.split_once('=')?;
                    key.trim()
                        .eq_ignore_ascii_case("q")
                        .then(|| quality(value.trim()))
                })
                .unwrap_or(Some(1000));
            let Some(weight) = weight else {
                continue;
            };
            if weight == 0 {
                continue;
            }
            listed = true;
            if let Some(locale) = Self::supported(tag)
                && best.is_none_or(|(top, _)| weight > top)
            {
                best = Some((weight, locale));
            }
        }
        match best {
            Some((_, locale)) => Some(locale),
            None if listed => Some(Self::FALLBACK),
            None => None,
        }
    }
}

/// Longest tag looked at (BCP 47 tags in practice stay far below).
const MAX_TAG: usize = 35;
/// Longest `Accept-Language` header looked at.
const MAX_HEADER: usize = 512;
/// Most entries of one header looked at.
const MAX_ITEMS: usize = 16;

/// A `q` value as thousandths (`"0.8"` → 800), `None` when malformed.
fn quality(value: &str) -> Option<u16> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if fraction.len() > 3 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let whole: u16 = match whole {
        "0" => 0,
        "1" => 1,
        _ => return None,
    };
    let mut thousandths = 0_u16;
    for (i, digit) in fraction.bytes().enumerate() {
        thousandths += u16::from(digit - b'0') * [100, 10, 1][i];
    }
    let value = whole * 1000 + thousandths;
    (value <= 1000).then_some(value)
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl FromStr for Locale {
    type Err = UnknownLocale;

    /// Exactly one of the stored codes (`"tr"`, …).
    fn from_str(code: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|locale| locale.code() == code)
            .ok_or(UnknownLocale)
    }
}

/// A stored code that is not one of [`Locale::ALL`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("unknown locale")]
pub struct UnknownLocale;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_map_to_one_of_the_five_else_english() {
        assert_eq!(Locale::from_tag("tr-TR"), Some(Locale::Tr));
        assert_eq!(Locale::from_tag("de_AT"), Some(Locale::De));
        assert_eq!(Locale::from_tag("FR"), Some(Locale::Fr));
        assert_eq!(Locale::from_tag("es-419"), Some(Locale::Es));
        assert_eq!(Locale::from_tag("en-GB"), Some(Locale::En));
        assert_eq!(Locale::from_tag("ja-JP"), Some(Locale::En));
        assert_eq!(Locale::from_tag(" "), None);
        assert_eq!(Locale::from_tag(&"x".repeat(80)), None);
    }

    #[test]
    fn accept_language_takes_the_best_supported_weight() {
        assert_eq!(
            Locale::from_accept_language("de-DE,de;q=0.9,en-US;q=0.8,en;q=0.7"),
            Some(Locale::De)
        );
        assert_eq!(
            Locale::from_accept_language("ja;q=1, es;q=0.4, fr;q=0.6"),
            Some(Locale::Fr)
        );
        assert_eq!(Locale::from_accept_language("tr, en"), Some(Locale::Tr));
        assert_eq!(
            Locale::from_accept_language("en;q=0.5, de"),
            Some(Locale::De)
        );
        assert_eq!(Locale::from_accept_language("ja, zh-CN"), Some(Locale::En));
        assert_eq!(Locale::from_accept_language("de;q=0, ja"), Some(Locale::En));
        assert_eq!(
            Locale::from_accept_language("de;q=bad, es"),
            Some(Locale::Es)
        );
        assert_eq!(Locale::from_accept_language(""), None);
        assert_eq!(Locale::from_accept_language("*"), None);
        assert_eq!(Locale::from_accept_language(&"de,".repeat(400)), None);
    }

    #[test]
    fn codes_round_trip() {
        for locale in Locale::ALL {
            assert_eq!(locale.code().parse::<Locale>(), Ok(locale));
            assert_eq!(
                serde_json::to_string(&locale).unwrap(),
                format!("\"{}\"", locale.code())
            );
        }
        assert_eq!("de-AT".parse::<Locale>(), Err(UnknownLocale));
    }
}
