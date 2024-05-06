//! Types and traits describing Minecraft block states.
//!
//! ## Example
//!
//! ```
//! # #[cfg(all(feature = "latest", feature = "serde", feature = "block-states"))]
//! # fn test() {
//! use mcdata::block_state::latest::{self, props};
//!
//! let banjo = latest::BlockState::NoteBlock {
//!     instrument: props::NoteBlockInstrument::Banjo,
//!     note: bounded_integer::BoundedU8::new(10).unwrap(),
//!     powered: false,
//! };
//! let banjo_nbt = fastnbt::nbt!({
//!     "Name": "minecraft:note_block",
//!     "Properties": {
//!         "instrument": "banjo",
//!         "note": "10",
//!         "powered": "false",
//!     },
//! });
//! assert_eq!(fastnbt::to_value(&banjo), Ok(banjo_nbt));
//! # }
//! # #[cfg(all(feature = "latest", feature = "serde", feature = "block-states"))]
//! # test();
//! ```

use std::collections::HashMap;

#[cfg(feature = "block-states")]
pub use self::list::*;

#[cfg(feature = "block-states")]
#[macro_use]
mod macros;
#[cfg(feature = "block-states")]
mod list;

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
    pub name: String,

    /// The properties of this block state as a map from names to values.
    #[cfg_attr(feature = "serde", serde(default))]
    pub properties: HashMap<String, String>,
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
