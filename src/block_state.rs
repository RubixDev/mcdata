//! Types and traits describing Minecraft block states.

use std::{borrow::Cow, collections::HashMap};

#[cfg(feature = "block-states")]
#[macro_use]
mod macros;
#[cfg(feature = "block-states")]
pub(crate) mod list;

/// Any type that can represent a block state.
pub trait BlockState: Clone + PartialEq + Sized {
    /// Return this type's representation of `minecraft:air`.
    fn air() -> Self;
}

/// A generic block state that can represent _any_ possible block state by storing the name and
/// properties as strings.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct GenericBlockState {
    /// The id of this block, e.g. `minecraft:air`.
    pub name: Cow<'static, str>,

    /// The properties of this block state as a map from names to values.
    #[cfg_attr(feature = "serde", serde(default))]
    pub properties: HashMap<Cow<'static, str>, Cow<'static, str>>,
}

impl BlockState for GenericBlockState {
    fn air() -> Self {
        Self {
            name: "minecraft:air".into(),
            properties: HashMap::new(),
        }
    }
}

impl BlockState for fastnbt::Value {
    fn air() -> Self {
        fastnbt::nbt!({ "Name": "minecraft:air" })
    }
}
