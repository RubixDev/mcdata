//! Types and traits describing Minecraft block entities, a.k.a. tile entities.

use std::{borrow::Cow, collections::HashMap};

#[cfg(feature = "serde")]
use std::{fmt, marker::PhantomData};

use crate::util::BlockPos;

/// Any type that can represent a block entity.
pub trait BlockEntity: Clone {
    /// Get the [`BlockPos`] of this block entity.
    fn position(&self) -> BlockPos;
}

/// A generic block entity that can represent _any_ block entity by storing its
/// [position](Self::pos) and [raw NBT](Self::properties).
#[derive(Clone, Debug, PartialEq)]
pub struct GenericBlockEntity<'a> {
    /// The [`BlockPos`] of this block entity.
    pub pos: BlockPos,
    /// The raw NBT properties of this block entity.
    pub properties: HashMap<Cow<'a, str>, fastnbt::Value>,
}

impl BlockEntity for GenericBlockEntity<'_> {
    fn position(&self) -> BlockPos {
        self.pos
    }
}

#[cfg(feature = "serde")]
impl<'de: 'a, 'a> serde::Deserialize<'de> for GenericBlockEntity<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct _Visitor<'de: 'a, 'a> {
            marker: PhantomData<GenericBlockEntity<'a>>,
            lifetime: PhantomData<&'de ()>,
        }
        impl<'de: 'a, 'a> serde::de::Visitor<'de> for _Visitor<'de, 'a> {
            type Value = GenericBlockEntity<'a>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "BlockEntity")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut x = None;
                let mut y = None;
                let mut z = None;
                let mut properties = HashMap::new();
                while let Some(key) = map.next_key::<Cow<'a, str>>()? {
                    match key.as_ref() {
                        "x" => {
                            if x.is_some() {
                                return Err(serde::de::Error::duplicate_field("x"));
                            }
                            x = Some(map.next_value()?);
                        }
                        "y" => {
                            if y.is_some() {
                                return Err(serde::de::Error::duplicate_field("y"));
                            }
                            y = Some(map.next_value()?);
                        }
                        "z" => {
                            if z.is_some() {
                                return Err(serde::de::Error::duplicate_field("z"));
                            }
                            z = Some(map.next_value()?);
                        }
                        _ => {
                            properties.insert(key, map.next_value()?);
                        }
                    }
                }
                let x = x.ok_or_else(|| serde::de::Error::missing_field("x"))?;
                let y = y.ok_or_else(|| serde::de::Error::missing_field("y"))?;
                let z = z.ok_or_else(|| serde::de::Error::missing_field("z"))?;
                Ok(Self::Value {
                    pos: BlockPos::new(x, y, z),
                    properties,
                })
            }
        }

        deserializer.deserialize_map(_Visitor {
            marker: PhantomData::<GenericBlockEntity<'a>>,
            lifetime: PhantomData,
        })
    }
}

#[cfg(feature = "serde")]
impl<'a> serde::Serialize for GenericBlockEntity<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut state = serializer.serialize_map(Some(self.properties.len() + 3))?;
        state.serialize_entry("x", &self.pos.x)?;
        state.serialize_entry("y", &self.pos.y)?;
        state.serialize_entry("z", &self.pos.z)?;
        for (key, value) in &self.properties {
            state.serialize_entry(key, value)?;
        }
        state.end()
    }
}
