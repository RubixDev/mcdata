//! Types and traits describing Minecraft block entities, a.k.a. tile entities.

use std::collections::HashMap;

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
pub struct GenericBlockEntity {
    /// The [`BlockPos`] of this block entity.
    pub pos: BlockPos,
    /// The raw NBT properties of this block entity.
    pub properties: HashMap<String, fastnbt::Value>,
}

impl BlockEntity for GenericBlockEntity {
    fn position(&self) -> BlockPos {
        self.pos
    }
}

impl BlockEntity for fastnbt::Value {
    fn position(&self) -> BlockPos {
        let Self::Compound(map) = self else {
            panic!("valid block entity should be a compound")
        };
        let Some(Self::Int(x)) = map.get("x") else {
            panic!("valid block entity has 'x' key of type int")
        };
        let Some(Self::Int(y)) = map.get("y") else {
            panic!("valid block entity has 'y' key of type int")
        };
        let Some(Self::Int(z)) = map.get("z") else {
            panic!("valid block entity has 'z' key of type int")
        };
        BlockPos::new(*x, *y, *z)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for GenericBlockEntity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct _Visitor<'de> {
            marker: PhantomData<GenericBlockEntity>,
            lifetime: PhantomData<&'de ()>,
        }
        impl<'de> serde::de::Visitor<'de> for _Visitor<'de> {
            type Value = GenericBlockEntity;

            fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt.write_str("BlockEntity")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut x = None;
                let mut y = None;
                let mut z = None;
                let mut properties = HashMap::new();
                while let Some(key) = map.next_key::<String>()? {
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
            marker: PhantomData::<GenericBlockEntity>,
            lifetime: PhantomData,
        })
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for GenericBlockEntity {
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
