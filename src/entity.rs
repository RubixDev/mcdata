//! Types and traits describing Minecraft entities.

use std::{borrow::Cow, collections::HashMap};

#[cfg(feature = "serde")]
use std::{fmt, marker::PhantomData};

/// Any type that can represent an entity.
pub trait Entity: Clone {}

/// A generic entity that can represent _any_ possible entity with state by storing its
/// [id](Self::id), [UUID](Self::uuid), and [raw NBT](Self::properties).
#[derive(Debug, Clone, PartialEq)]
pub struct GenericEntity<'a> {
    /// The id of this entity, e.g. `minecraft:cow`.
    pub id: Cow<'a, str>,
    /// The UUID of this entity, stored as a 128-bit integer.
    pub uuid: u128,
    /// The raw NBT properties of this entity.
    pub properties: HashMap<Cow<'a, str>, fastnbt::Value>,
}

impl Entity for GenericEntity<'_> {}

#[cfg(feature = "serde")]
impl<'de: 'a, 'a> serde::Deserialize<'de> for GenericEntity<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct _Visitor<'de: 'a, 'a> {
            marker: PhantomData<GenericEntity<'a>>,
            lifetime: PhantomData<&'de ()>,
        }
        impl<'de: 'a, 'a> serde::de::Visitor<'de> for _Visitor<'de, 'a> {
            type Value = GenericEntity<'a>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "Entity")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut id = None;
                let mut uuid = None;
                let mut properties = HashMap::new();
                while let Some(key) = map.next_key::<Cow<'a, str>>()? {
                    match key.as_ref() {
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
            marker: PhantomData::<GenericEntity<'a>>,
            lifetime: PhantomData,
        })
    }
}

#[cfg(feature = "serde")]
impl<'a> serde::Serialize for GenericEntity<'a> {
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
