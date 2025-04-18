use serde::de::{self, Deserializer, MapAccess, SeqAccess, Unexpected, Visitor};
use serde::Deserialize;
use std::fmt;

pub type EntryId = i64;
pub type MenuId = i64;

pub fn deserialize_flexible_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    struct FlexibleI64Visitor;
    impl<'de> Visitor<'de> for FlexibleI64Visitor {
        type Value = i64;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("integer or string int")
        }
        #[inline]
        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
            Ok(v)
        }
        #[inline]
        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            i64::try_from(v).map_err(|_| E::invalid_value(Unexpected::Unsigned(v), &self))
        }
        #[inline]
        fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if v.fract() == 0.0 && v >= i64::MIN as f64 && v <= i64::MAX as f64 {
                Ok(v as i64)
            } else {
                Err(E::invalid_value(
                    Unexpected::Float(v),
                    &"whole number float",
                ))
            }
        }
        #[inline]
        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            v.trim()
                .parse::<i64>()
                .map_err(|_| E::invalid_value(Unexpected::Str(v), &"string int"))
        }
    }
    deserializer.deserialize_any(FlexibleI64Visitor)
}

pub fn deserialize_optional_flexible_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    struct OptionalFlexibleI64Visitor;
    impl<'de> Visitor<'de> for OptionalFlexibleI64Visitor {
        type Value = Option<i64>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("integer, null, empty string, or string int")
        }

        #[inline]
        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
            Ok(Some(v))
        }
        #[inline]
        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            i64::try_from(v)
                .map(Some)
                .map_err(|_| E::invalid_value(Unexpected::Unsigned(v), &self))
        }
        #[inline]
        fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if v.fract() == 0.0 && v >= i64::MIN as f64 && v <= i64::MAX as f64 {
                Ok(Some(v as i64))
            } else {
                Ok(None)
            }
        }
        #[inline]
        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let t = v.trim();
            if t.is_empty() {
                Ok(None)
            } else {
                Ok(t.parse::<i64>().ok())
            }
        }
        #[inline]
        fn visit_none<E>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        #[inline]
        fn visit_unit<E>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        #[inline]
        fn visit_bool<E>(self, _v: bool) -> Result<Self::Value, E> {
            Ok(None)
        }
        #[inline]
        fn visit_map<A>(self, _map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            Ok(None)
        }
        #[inline]
        fn visit_seq<A>(self, _seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            Ok(None)
        }
    }
    deserializer.deserialize_any(OptionalFlexibleI64Visitor)
}

pub fn deserialize_optional_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let v: Option<String> = Option::deserialize(deserializer)?;
    Ok(v.filter(|s| !s.trim().is_empty()))
}

pub fn deserialize_string_or_default<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrDefaultVisitor;

    impl<'de> Visitor<'de> for StringOrDefaultVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string, null, or empty array")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value.to_string())
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value)
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(String::new())
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(String::new())
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            if seq.next_element::<de::IgnoredAny>()?.is_none() {
                Ok(String::new())
            } else {
                Ok(String::new())
            }
        }
    }

    deserializer.deserialize_any(StringOrDefaultVisitor)
}
