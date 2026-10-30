//! Screen share resolution and frame-rate limits.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::limit::{LimitError, LimitKey, LimitValue};

/// Screen share output resolution (architecture.md §7.3), ordered from lowest
/// to highest.
///
/// [`StreamResolution::Source`] means "no downscaling" and sorts above every
/// concrete resolution, so only plans whose top tier is `Source` allow it.
/// Clients that know the captured size should request the matching concrete
/// resolution instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum StreamResolution {
    /// 1280×720.
    #[serde(rename = "720p")]
    P720,
    /// 1920×1080.
    #[serde(rename = "1080p")]
    P1080,
    /// 2560×1440.
    #[serde(rename = "1440p")]
    P1440,
    /// 3840×2160 (4K).
    #[serde(rename = "2160p")]
    P2160,
    /// The captured source's native resolution.
    #[serde(rename = "source")]
    Source,
}

impl StreamResolution {
    /// Vertical pixel count, or `None` for [`StreamResolution::Source`].
    #[must_use]
    pub const fn height(self) -> Option<u32> {
        match self {
            Self::P720 => Some(720),
            Self::P1080 => Some(1080),
            Self::P1440 => Some(1440),
            Self::P2160 => Some(2160),
            Self::Source => None,
        }
    }

    /// The wire name (`"720p"`, …, `"source"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::P720 => "720p",
            Self::P1080 => "1080p",
            Self::P1440 => "1440p",
            Self::P2160 => "2160p",
            Self::Source => "source",
        }
    }
}

impl fmt::Display for StreamResolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A resolution together with a frame rate, e.g. 1080p at 60 fps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamQuality {
    /// Output resolution.
    pub resolution: StreamResolution,
    /// Frames per second.
    pub fps: u32,
}

impl StreamQuality {
    /// Creates a quality from a resolution and frame rate.
    #[must_use]
    pub const fn new(resolution: StreamResolution, fps: u32) -> Self {
        Self { resolution, fps }
    }

    /// Whether this quality is at or below `cap` in both resolution and fps.
    #[must_use]
    pub fn fits_within(self, cap: Self) -> bool {
        self.resolution <= cap.resolution && self.fps <= cap.fps
    }
}

impl fmt::Display for StreamQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.resolution, self.fps)
    }
}

/// The highest stream qualities a plan allows, as one or more tiers.
///
/// A request is allowed when it fits within at least one tier. Most plans have
/// one tier; Community has two ("1440p60 / 4K30" in §12.1), so 4K is allowed
/// at 30 fps but not at 60. Always holds at least one tier; to forbid
/// streaming entirely, set the concurrent stream limit to zero.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StreamLimit {
    tiers: Vec<StreamQuality>,
}

impl StreamLimit {
    /// A limit with a single tier.
    #[must_use]
    pub fn new(tier: StreamQuality) -> Self {
        Self { tiers: vec![tier] }
    }

    /// Adds an alternative tier (builder style): `new(1440p60).or(2160p30)`.
    #[must_use]
    pub fn or(mut self, tier: StreamQuality) -> Self {
        self.tiers.push(tier);
        self
    }

    /// The tiers, in insertion order.
    #[must_use]
    pub fn tiers(&self) -> &[StreamQuality] {
        &self.tiers
    }

    /// Whether `resolution` at `fps` fits within any tier.
    #[must_use]
    pub fn allows(&self, resolution: StreamResolution, fps: u32) -> bool {
        let requested = StreamQuality::new(resolution, fps);
        self.tiers.iter().any(|&tier| requested.fits_within(tier))
    }

    /// The highest frame rate allowed at `resolution`, or `None` if the
    /// resolution itself is not allowed. Useful for building the stream
    /// settings UI (§7.3).
    #[must_use]
    pub fn max_fps_for(&self, resolution: StreamResolution) -> Option<u32> {
        self.tiers
            .iter()
            .filter(|tier| resolution <= tier.resolution)
            .map(|tier| tier.fps)
            .max()
    }

    /// The highest resolution allowed at any frame rate.
    #[must_use]
    pub fn max_resolution(&self) -> StreamResolution {
        self.tiers
            .iter()
            .map(|tier| tier.resolution)
            .max()
            .unwrap_or(StreamResolution::P720)
    }

    /// Checks a requested quality.
    ///
    /// If some tier covers the resolution, the violation is reported as
    /// [`LimitKey::MaxStreamFps`] with the best fps for that resolution;
    /// otherwise as [`LimitKey::MaxStreamResolution`].
    pub fn check(&self, resolution: StreamResolution, fps: u32) -> Result<(), LimitError> {
        if self.allows(resolution, fps) {
            return Ok(());
        }
        Err(match self.max_fps_for(resolution) {
            Some(max_fps) => LimitError {
                key: LimitKey::MaxStreamFps,
                limit: LimitValue::Number(u64::from(max_fps)),
                requested: LimitValue::Number(u64::from(fps)),
            },
            None => LimitError {
                key: LimitKey::MaxStreamResolution,
                limit: LimitValue::Resolution(self.max_resolution()),
                requested: LimitValue::Resolution(resolution),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use StreamResolution::*;

    fn community() -> StreamLimit {
        StreamLimit::new(StreamQuality::new(P1440, 60)).or(StreamQuality::new(P2160, 30))
    }

    #[test]
    fn resolutions_are_ordered_with_source_on_top() {
        assert!(P720 < P1080 && P1080 < P1440 && P1440 < P2160 && P2160 < Source);
        assert_eq!(P2160.height(), Some(2160));
        assert_eq!(Source.height(), None);
    }

    #[test]
    fn resolution_serde_names() {
        for (resolution, name) in [
            (P720, "720p"),
            (P1080, "1080p"),
            (P1440, "1440p"),
            (P2160, "2160p"),
            (Source, "source"),
        ] {
            let json = format!("\"{name}\"");
            assert_eq!(serde_json::to_string(&resolution).unwrap(), json);
            assert_eq!(
                serde_json::from_str::<StreamResolution>(&json).unwrap(),
                resolution
            );
            assert_eq!(resolution.to_string(), name);
        }
    }

    #[test]
    fn quality_serde_round_trip() {
        let quality = StreamQuality::new(P1080, 60);
        let json = serde_json::to_string(&quality).unwrap();
        assert_eq!(json, r#"{"resolution":"1080p","fps":60}"#);
        assert_eq!(
            serde_json::from_str::<StreamQuality>(&json).unwrap(),
            quality
        );
        assert_eq!(quality.to_string(), "1080p@60");
    }

    #[test]
    fn multi_tier_limit_allows_either_tier() {
        let limit = community();
        assert!(limit.allows(P1440, 60));
        assert!(limit.allows(P1080, 60));
        assert!(limit.allows(P2160, 30));
        assert!(limit.allows(P2160, 15));
        assert!(!limit.allows(P2160, 60));
        assert!(!limit.allows(Source, 30));
        assert_eq!(limit.max_fps_for(P1080), Some(60));
        assert_eq!(limit.max_fps_for(P2160), Some(30));
        assert_eq!(limit.max_fps_for(Source), None);
        assert_eq!(limit.max_resolution(), P2160);
        assert_eq!(limit.tiers().len(), 2);
    }

    #[test]
    fn check_reports_fps_when_resolution_is_covered() {
        assert_eq!(
            community().check(P2160, 60),
            Err(LimitError {
                key: LimitKey::MaxStreamFps,
                limit: LimitValue::Number(30),
                requested: LimitValue::Number(60),
            })
        );
    }

    #[test]
    fn check_reports_resolution_when_no_tier_covers_it() {
        assert_eq!(
            community().check(Source, 15),
            Err(LimitError {
                key: LimitKey::MaxStreamResolution,
                limit: LimitValue::Resolution(P2160),
                requested: LimitValue::Resolution(Source),
            })
        );
    }

    #[test]
    fn source_tier_allows_everything_up_to_its_fps() {
        let limit = StreamLimit::new(StreamQuality::new(Source, 60));
        for resolution in [P720, P1080, P1440, P2160, Source] {
            assert_eq!(limit.check(resolution, 60), Ok(()));
        }
        assert_eq!(
            limit.check(Source, 61).unwrap_err().key,
            LimitKey::MaxStreamFps
        );
    }
}
