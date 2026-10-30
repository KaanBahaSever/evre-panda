//! Serde helpers shared by several wire types.

/// `u64` encoded as a decimal string, e.g. `"1234"` (JavaScript-safe).
pub(crate) mod u64_string {
    use std::fmt;

    use serde::de::{self, Unexpected, Visitor};
    use serde::{Deserializer, Serializer};

    pub(crate) fn serialize<S: Serializer>(value: u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&value)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        deserializer.deserialize_str(DecimalStringVisitor)
    }

    struct DecimalStringVisitor;

    impl Visitor<'_> for DecimalStringVisitor {
        type Value = u64;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an unsigned 64-bit integer as a decimal string")
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<u64, E> {
            value
                .parse()
                .map_err(|_| E::invalid_value(Unexpected::Str(value), &self))
        }
    }
}

/// `Option<Option<T>>` for PATCH bodies: an absent field is `None` (keep the
/// current value), an explicit `null` is `Some(None)` (clear it) and a value is
/// `Some(Some(v))`. Use with `#[serde(default, skip_serializing_if =
/// "Option::is_none", with = "double_option")]`.
pub(crate) mod double_option {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(crate) fn serialize<S, T>(
        value: &Option<Option<T>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Serialize,
    {
        match value {
            Some(inner) => inner.serialize(serializer),
            None => serializer.serialize_none(),
        }
    }

    pub(crate) fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        Option::<T>::deserialize(deserializer).map(Some)
    }
}
