# mcdata

A Rust library providing traits and types representing various Minecraft NBT
structures.

## Overview

Currently, this crate provides three traits along with some implementations of
those traits:

- [`BlockState`] for block states. See the [`block_state`] module for more.
- [`Entity`] for normal entities. See the [`entity`] module for more.
- [`BlockEntity`] for block entities. See the [`block_entity`] module for more.

There's one "generic" implementation for each of these in the corresponding
module. Other implementations are locked behind [features](#features).

With `serde` support enabled, all types in this crate can be full serialized and
deserialized from/to NBT. The recommended crate for that is
[`fastnbt`](https://crates.io/crates/fastnbt) which is also internally used by
the types in this crate. For example [`entity::GenericEntity`] is almost fully
represented by a [`fastnbt::Value`].
