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

## Where does this data come from?

As you can imagine, I did not enter all this data by hand. It is automatically
extracted from the Minecraft jars using a combination of two custom tools:

1. The
   ["data-extractor"](https://github.com/RubixDev/mcdata/tree/main/data-extractor)
   is a Java file that acts as a Fabric mod and gets some data at runtime using
   reflection. This includes everything about block states, and some things
   about entities (the IDs of all entities and their corresponding classes).
2. The (badly named)
   ["class-parser"](https://github.com/RubixDev/mcdata/tree/main/class-parser)
   is almost like a JVM written in Kotlin which takes the data about entities
   from the "data-extractor" and "interprets" the Minecraft jars to gather
   information about their NBT structure. This is necessary because entity NBT
   is unstructured and the only thing that defines the structure is the actual
   code itself. This is obviously still a brittle approach, but the best I could
   think of. Some things are also very difficult to properly support in this
   "JVM", mainly Minecraft's `Codec`s which are getting used more and more with
   new Minecraft updates. This together means that all types in this crate which
   describe NBT structure _might_ not be fully correct. If you find any
   discrepancies, please
   [open an issue ticket](https://github.com/RubixDev/mcdata/issues).

If you want to contribute to `mcdata` in any way that involves changing the
generated code, you can run these tools along with the actual code generation
using `cargo xtask codegen`. Just make sure you have a working Java 21 JDK,
`git`, and [`deno`](https://deno.com/) installed and on your PATH.
