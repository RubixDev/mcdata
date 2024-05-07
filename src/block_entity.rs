//! Types and traits describing Minecraft block entities, a.k.a. tile entities.
//!
//! ## Example
//!
//! ```
//! # #[cfg(all(feature = "latest", feature = "serde", feature = "block-entities"))]
//! # fn test() {
//! use mcdata::block_entity::latest::{self, types};
//!
//! let command_block = latest::BlockEntity::CommandBlock(types::CommandBlockEntity {
//!     command: "/say hi".to_string(),
//!     custom_name: None,
//!     last_execution: None,
//!     last_output: None,
//!     success_count: 2,
//!     track_output: true,
//!     update_last_execution: true,
//!     auto: false,
//!     condition_met: false,
//!     powered: false,
//!     parent: types::BlockEntity {
//!         x: 0,
//!         y: 10,
//!         z: -5,
//!     },
//! });
//! let command_block_nbt = fastnbt::nbt!({
//!     "id": "minecraft:command_block",
//!     "Command": "/say hi",
//!     "SuccessCount": 2,
//!     "TrackOutput": true,
//!     "UpdateLastExecution": true,
//!     "auto": false,
//!     "conditionMet": false,
//!     "powered": false,
//!     "x": 0,
//!     "y": 10,
//!     "z": -5,
//! });
//! assert_eq!(fastnbt::to_value(&command_block), Ok(command_block_nbt));
//! # }
//! # #[cfg(all(feature = "latest", feature = "serde", feature = "block-entities"))]
//! # test();
//! ```

use std::collections::HashMap;

#[cfg(feature = "serde")]
use std::{fmt, marker::PhantomData};

use crate::util::BlockPos;

#[cfg(feature = "block-entities")]
pub use self::list::*;

#[cfg(feature = "block-entities")]
#[macro_use]
mod macros;
#[cfg(feature = "block-entities")]
mod list;

/// Any type that can represent a block entity.
pub trait BlockEntity: Clone {
    /// Get the [`BlockPos`] of this block entity.
    fn position(&self) -> BlockPos;
}

/// A generic block entity that can represent _any_ block entity by storing its
/// [position](Self::pos) and [raw NBT](Self::properties).
#[derive(Clone, Debug, PartialEq)]
pub struct GenericBlockEntity {
    /// The ID of this block entity.
    ///
    /// Note that litematica had a bug [introduced in version `1.18.0-0.9.0`](https://github.com/maruohon/litematica/commit/8f58911524852b5c8edeb8b185ec5751201599a2#diff-334964871b9057033353e22bf7656fa45612087ccca6d59dee71cb8956e9a304)
    /// which was only [fixed in `1.20.1-0.15.3`](https://github.com/maruohon/litematica/commit/a156bf6ba80f81196b62aaa069589c5c4010fabe)
    /// that caused the block entity IDs to not be included in saved schematics. When deserializing
    /// from one such schematic, this field will be an empty string.
    pub id: String,

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
            panic!("valid block entity should be a compound");
        };
        let Some(Self::Int(x)) = map.get("x") else {
            panic!("valid block entity has 'x' key of type int");
        };
        let Some(Self::Int(y)) = map.get("y") else {
            panic!("valid block entity has 'y' key of type int");
        };
        let Some(Self::Int(z)) = map.get("z") else {
            panic!("valid block entity has 'z' key of type int");
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
                let mut id = None;
                let mut x = None;
                let mut y = None;
                let mut z = None;
                let mut properties = HashMap::new();
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_ref() {
                        "id" => {
                            if id.is_some() {
                                return Err(serde::de::Error::duplicate_field("id"));
                            }
                            id = Some(map.next_value()?);
                        }
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
                let id = id.unwrap_or_default();
                let x = x.ok_or_else(|| serde::de::Error::missing_field("x"))?;
                let y = y.ok_or_else(|| serde::de::Error::missing_field("y"))?;
                let z = z.ok_or_else(|| serde::de::Error::missing_field("z"))?;
                Ok(Self::Value {
                    id,
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
        if !self.id.is_empty() {
            state.serialize_entry("id", &self.id)?;
        }
        state.serialize_entry("x", &self.pos.x)?;
        state.serialize_entry("y", &self.pos.y)?;
        state.serialize_entry("z", &self.pos.z)?;
        for (key, value) in &self.properties {
            state.serialize_entry(key, value)?;
        }
        state.end()
    }
}
