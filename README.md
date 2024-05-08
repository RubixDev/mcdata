# mcdata

A Rust library providing traits and types representing various Minecraft NBT
structures.

## Overview

Currently, this crate provides three traits along with some implementations of
those traits:

- [`BlockState`] for block states.
- [`Entity`] for normal entities.
- [`BlockEntity`] for block entities.

There's one "generic" implementation for each of these in the corresponding
module. Other implementations are locked behind [features](#features).

With `serde` support enabled, all types in this crate can be full serialized and
deserialized from/to NBT. The recommended crate for that is
[`fastnbt`](https://crates.io/crates/fastnbt) which is also internally used by
the types in this crate. For example [`entity::GenericEntity`] is almost fully
represented by a [`fastnbt::Value`].

## A Warning for Block Entities

If you intend to use any version-specific `BlockEntity` type with litematica
files, beware that there was a bug in litematica since version `1.18.0-0.9.0`
that was only fixed in `1.20.1-0.15.3` which caused the block entity IDs to not
be included in saved schematics. The block entity types _do_ support
deserialization without an ID, but that can never be perfect. For instance,
[barrels](mc1_18::block_entity_types::BarrelBlockEntity) and
[chests](mc1_18::block_entity_types::ChestBlockEntity) in 1.18 have the same
exact NBT structure, but barrels will always be tested first when deserializing,
so chests will also be deserialized as barrels when there's no ID distinguishing
the two. During serialization, the ID will always be included, so such a chest
would become a barrel by just reading and writing the NBT. If that's a problem
for you, consider using [`GenericBlockEntity`] instead which won't mess with
missing IDs.

## Examples

<details>
<summary><strong>Block States</strong></summary>

```rust
# #[cfg(not(feature = "test"))]
# compile_error!("tests should be run with the 'test' feature enabled");
use mcdata::latest::{BlockState, props};

let banjo = BlockState::NoteBlock {
    instrument: props::NoteBlockInstrument::Banjo,
    note: bounded_integer::BoundedU8::new(10).unwrap(),
    powered: false,
};
let banjo_nbt = fastnbt::nbt!({
    "Name": "minecraft:note_block",
    "Properties": {
        "instrument": "banjo",
        "note": "10",
        "powered": "false",
    },
});
assert_eq!(fastnbt::to_value(&banjo), Ok(banjo_nbt));
```

</details>

<details>
<summary><strong>Entities</strong></summary>

```rust
# #[cfg(not(feature = "test"))]
# compile_error!("tests should be run with the 'test' feature enabled");
use std::collections::HashMap;
use mcdata::latest::{Entity, entity_types as types, entity_compounds as compounds};

let axolotl = Entity::Axolotl(types::Axolotl {
    from_bucket: false,
    variant: 0,
    parent: types::Animal {
        in_love: 0,
        love_cause: None,
        parent: types::AgeableMob {
            age: 0,
            forced_age: 0,
            parent: types::PathfinderMob {
                parent: types::Mob {
                    armor_drop_chances: vec![0.085; 4],
                    armor_items: vec![HashMap::new(); 4],
                    can_pick_up_loot: false,
                    death_loot_table: None,
                    body_armor_drop_chance: None,
                    death_loot_table_seed: None,
                    hand_drop_chances: vec![0.085; 2],
                    hand_items: vec![HashMap::new(); 2],
                    left_handed: false,
                    no_ai: None,
                    persistence_required: false,
                    body_armor_item: None,
                    leash: None,
                    parent: types::LivingEntity {
                        absorption_amount: 0.,
                        attributes: vec![compounds::AttributeInstance_save {
                            base: 1.,
                            modifiers: None,
                            name: "minecraft:generic.movement_speed".into(),
                        }],
                        brain: Some(fastnbt::nbt!({ "memories": {} })),
                        death_time: 0,
                        fall_flying: false,
                        health: 14.,
                        hurt_by_timestamp: 0,
                        hurt_time: 0,
                        sleeping_x: None,
                        sleeping_y: None,
                        sleeping_z: None,
                        active_effects: None,
                        parent: types::Entity {
                            air: 6000,
                            custom_name: None,
                            custom_name_visible: None,
                            fall_distance: 0.,
                            fire: -1,
                            glowing: None,
                            has_visual_fire: None,
                            invulnerable: false,
                            motion: vec![0.; 3],
                            no_gravity: None,
                            on_ground: false,
                            passengers: None,
                            portal_cooldown: 0,
                            pos: vec![-0.5, 0., 1.5],
                            rotation: vec![-107.68715, 0.],
                            silent: None,
                            tags: None,
                            ticks_frozen: None,
                            uuid: 307716075036743941152627606223512221703,
                        },
                    },
                },
            },
        },
    },
});
let axolotl_nbt = fastnbt::nbt!({
    "id": "minecraft:axolotl",
    "FromBucket": false,
    "Variant": 0,
    "InLove": 0,
    "Age": 0,
    "ForcedAge": 0,
    "ArmorDropChances": vec![0.085_f32; 4],
    "ArmorItems": [{}, {}, {}, {}],
    "CanPickUpLoot": false,
    "HandDropChances": vec![0.085_f32; 2],
    "HandItems": [{}, {}],
    "LeftHanded": false,
    "PersistenceRequired": false,
    "AbsorptionAmount": 0_f32,
    "Attributes": [{
        "Base": 1.,
        "Name": "minecraft:generic.movement_speed",
    }],
    "Brain": { "memories": {} },
    "DeathTime": 0_i16,
    "FallFlying": false,
    "Health": 14_f32,
    "HurtByTimestamp": 0,
    "HurtTime": 0_i16,
    "Air": 6000_i16,
    "FallDistance": 0_f32,
    "Fire": -1_i16,
    "Invulnerable": false,
    "Motion": vec![0.; 3],
    "OnGround": false,
    "PortalCooldown": 0,
    "Pos": [-0.5, 0., 1.5],
    "Rotation": [-107.68715_f32, 0_f32],
    "UUID": [I; -411044392, 312166398, -1883713137, 1472542727],
});
assert_eq!(fastnbt::to_value(&axolotl), Ok(axolotl_nbt));
```

</details>

<details>
<summary><strong>Block Entities</strong></summary>

```rust
# #[cfg(not(feature = "test"))]
# compile_error!("tests should be run with the 'test' feature enabled");
use mcdata::latest::{BlockEntity, block_entity_types as types};

let command_block = BlockEntity::CommandBlock(types::CommandBlockEntity {
    command: "/say hi".into(),
    custom_name: None,
    last_execution: None,
    last_output: None,
    success_count: 2,
    track_output: true,
    update_last_execution: true,
    auto: false,
    condition_met: false,
    powered: false,
    parent: types::BlockEntity {
        x: 0,
        y: 10,
        z: -5,
    },
});
let command_block_nbt = fastnbt::nbt!({
    "id": "minecraft:command_block",
    "Command": "/say hi",
    "SuccessCount": 2,
    "TrackOutput": true,
    "UpdateLastExecution": true,
    "auto": false,
    "conditionMet": false,
    "powered": false,
    "x": 0,
    "y": 10,
    "z": -5,
});
assert_eq!(fastnbt::to_value(&command_block), Ok(command_block_nbt));
```

</details>

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
