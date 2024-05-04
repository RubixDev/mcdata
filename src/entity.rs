//! Types and traits describing Minecraft entities.

use std::collections::HashMap;

#[cfg(feature = "serde")]
use std::{fmt, marker::PhantomData};

#[cfg(feature = "entities")]
pub use self::list::*;

#[cfg(feature = "entities")]
#[macro_use]
mod macros;
#[cfg(feature = "entities")]
mod list;

/// Any type that can represent an entity.
pub trait Entity: Clone {}

/// A generic entity that can represent _any_ possible entity with state by storing its
/// [id](Self::id), [UUID](Self::uuid), and [raw NBT](Self::properties).
// TODO: try to make this use `Cow<'a, str>` again
#[derive(Debug, Clone, PartialEq)]
pub struct GenericEntity {
    /// The id of this entity, e.g. `minecraft:cow`.
    pub id: String,
    /// The UUID of this entity, stored as a 128-bit integer.
    pub uuid: u128,
    /// The raw NBT properties of this entity.
    pub properties: HashMap<String, fastnbt::Value>,
}

impl Entity for GenericEntity {}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for GenericEntity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct _Visitor<'de> {
            marker: PhantomData<GenericEntity>,
            lifetime: PhantomData<&'de ()>,
        }
        impl<'de> serde::de::Visitor<'de> for _Visitor<'de> {
            type Value = GenericEntity;

            fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt.write_str("Entity")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut id = None;
                let mut uuid = None;
                let mut properties = HashMap::new();
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "id" => {
                            if id.is_some() {
                                return Err(serde::de::Error::duplicate_field("id"));
                            }
                            id = Some(map.next_value()?);
                        }
                        "UUID" => {
                            if uuid.is_some() {
                                return Err(serde::de::Error::duplicate_field("UUID"));
                            }
                            uuid = Some(map.next_value()?);
                        }
                        _ => {
                            properties.insert(key, map.next_value()?);
                        }
                    }
                }
                let id = id.ok_or_else(|| serde::de::Error::missing_field("id"))?;
                let uuid = uuid.ok_or_else(|| serde::de::Error::missing_field("UUID"))?;
                Ok(Self::Value {
                    id,
                    uuid,
                    properties,
                })
            }
        }

        deserializer.deserialize_map(_Visitor {
            marker: PhantomData::<GenericEntity>,
            lifetime: PhantomData,
        })
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for GenericEntity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut state = serializer.serialize_map(Some(self.properties.len() + 2))?;
        state.serialize_entry("id", &self.id)?;
        state.serialize_entry("UUID", &self.uuid)?;
        for (key, value) in &self.properties {
            state.serialize_entry(key, value)?;
        }
        state.end()
    }
}
