//! The legal texts a new account accepts (architecture.md §11.7), in one
//! place for the server and every client.
//!
//! Registration sends the versions the person accepted
//! ([`RegisterRequest::terms_version`](super::RegisterRequest::terms_version)
//! and [`privacy_version`](super::RegisterRequest::privacy_version)); the
//! server refuses any other version, so bumping a constant here makes older
//! clients ask for an update. Asking existing accounts again after a bump
//! is later work.
//!
//! 2026-10-28: all three texts revised (clarifications that match what the
//! service already does; architecture.md §14 "28 Ekim revizyonu"), so
//! accounts that accepted `2026-10-25` are not asked again.
//!
//! 2026-10-29: the terms and the privacy policy revised for account
//! deletion, reports and the two-year retention of deleted content
//! (architecture.md §14 "29 Ekim revizyonu", ADR-091). The Açık Rıza Metni
//! did not change, so [`CONSENT_VERSION`] stays `2026-10-28` (the Android app
//! compares its stored push consent with it).
//!
//! 2026-10-30: the privacy policy and the Açık Rıza Metni revised for the
//! password reset (ADR-103) and the in-app e-mail consent switch (ADR-104);
//! the recipients, data and purposes did not widen, so nobody is asked
//! again (architecture.md §14 "30 Ekim revizyonu"). The push part of the
//! Açık Rıza Metni did not change, so [`PUSH_CONSENT_VERSION`] stays
//! `2026-10-28` and the Android app does not ask for the push consent again.

/// Earlier versions the server still takes at registration, recording the
/// current one: the 0.8.0 apps went out with `2026-10-25` hours before the
/// 28 Ekim revision and the 0.8.x apps with `2026-10-28`, and every client
/// only links to the live pages, so the person read the current text either
/// way. A version belongs here only when its successor added no obligation
/// for the person (architecture.md §14).
pub const STILL_ACCEPTED: &[&str] = &["2026-10-25", "2026-10-28", "2026-10-29"];

/// Version of the Kullanım Koşulları (terms of use).
pub const TERMS_VERSION: &str = "2026-10-29";

/// Version of the Gizlilik Politikası ve KVKK Aydınlatma Metni (privacy
/// policy and the KVKK information notice).
pub const PRIVACY_VERSION: &str = "2026-10-30";

/// Version of the Açık Rıza Metni (the explicit consent to transferring the
/// e-mail address to Resend, US). The server records it with the consent.
pub const CONSENT_VERSION: &str = "2026-10-30";

/// The version of the Açık Rıza Metni whose push part (Android
/// notifications, Google Firebase) the apps ask for and register with
/// (ADR-087). It moves only when that part changes, so a revision of the
/// e-mail part does not ask everyone for the push consent again.
pub const PUSH_CONSENT_VERSION: &str = "2026-10-28";

/// Where the terms of use are published.
pub const TERMS_URL: &str = "https://evre.chat/kosullar/";

/// Where the privacy policy is published.
pub const PRIVACY_URL: &str = "https://evre.chat/gizlilik/";

/// Where the Açık Rıza Metni (the explicit consent to transferring the
/// e-mail address to Resend, US) is published.
pub const CONSENT_URL: &str = "https://evre.chat/acik-riza/";
