//! Types and traits describing Minecraft entities.
//!
//! ## Example
//!
//! ```
//! # #[cfg(all(feature = "latest", feature = "serde", feature = "entities"))]
//! # fn test() {
//! use std::collections::HashMap;
//! use mcdata::entity::latest::{self, compounds, types};
//!
//! let axolotl = latest::Entity::Axolotl(types::Axolotl {
//!     from_bucket: false,
//!     variant: 0,
//!     parent: types::Animal {
//!         in_love: 0,
//!         love_cause: None,
//!         parent: types::AgeableMob {
//!             age: 0,
//!             forced_age: 0,
//!             parent: types::PathfinderMob {
//!                 parent: types::Mob {
//!                     armor_drop_chances: vec![0.085; 4],
//!                     armor_items: vec![HashMap::new(); 4],
//!                     can_pick_up_loot: false,
//!                     death_loot_table: None,
//!                     body_armor_drop_chance: None,
//!                     death_loot_table_seed: None,
//!                     hand_drop_chances: vec![0.085; 2],
//!                     hand_items: vec![HashMap::new(); 2],
//!                     left_handed: false,
//!                     no_ai: None,
//!                     persistence_required: false,
//!                     body_armor_item: None,
//!                     leash: None,
//!                     parent: types::LivingEntity {
//!                         absorption_amount: 0.,
//!                         attributes: vec![compounds::AttributeInstance_save {
//!                             base: 1.,
//!                             modifiers: None,
//!                             name: "minecraft:generic.movement_speed".to_string(),
//!                         }],
//!                         brain: Some(fastnbt::nbt!({ "memories": {} })),
//!                         death_time: 0,
//!                         fall_flying: false,
//!                         health: 14.,
//!                         hurt_by_timestamp: 0,
//!                         hurt_time: 0,
//!                         sleeping_x: None,
//!                         sleeping_y: None,
//!                         sleeping_z: None,
//!                         active_effects: None,
//!                         parent: types::Entity {
//!                             air: 6000,
//!                             custom_name: None,
//!                             custom_name_visible: None,
//!                             fall_distance: 0.,
//!                             fire: -1,
//!                             glowing: None,
//!                             has_visual_fire: None,
//!                             invulnerable: false,
//!                             motion: vec![0.; 3],
//!                             no_gravity: None,
//!                             on_ground: false,
//!                             passengers: None,
//!                             portal_cooldown: 0,
//!                             pos: vec![-0.5, 0., 1.5],
//!                             rotation: vec![-107.68715, 0.],
//!                             silent: None,
//!                             tags: None,
//!                             ticks_frozen: None,
//!                             uuid: 307716075036743941152627606223512221703,
//!                         },
//!                     },
//!                 },
//!             },
//!         },
//!     },
//! });
//! let axolotl_nbt = fastnbt::nbt!({
//!     "id": "minecraft:axolotl",
//!     "FromBucket": false,
//!     "Variant": 0,
//!     "InLove": 0,
//!     "Age": 0,
//!     "ForcedAge": 0,
//!     "ArmorDropChances": vec![0.085_f32; 4],
//!     "ArmorItems": [{}, {}, {}, {}],
//!     "CanPickUpLoot": false,
//!     "HandDropChances": vec![0.085_f32; 2],
//!     "HandItems": [{}, {}],
//!     "LeftHanded": false,
//!     "PersistenceRequired": false,
//!     "AbsorptionAmount": 0_f32,
//!     "Attributes": [{
//!         "Base": 1.,
//!         "Name": "minecraft:generic.movement_speed",
//!     }],
//!     "Brain": { "memories": {} },
//!     "DeathTime": 0_i16,
//!     "FallFlying": false,
//!     "Health": 14_f32,
//!     "HurtByTimestamp": 0,
//!     "HurtTime": 0_i16,
//!     "Air": 6000_i16,
//!     "FallDistance": 0_f32,
//!     "Fire": -1_i16,
//!     "Invulnerable": false,
//!     "Motion": vec![0.; 3],
//!     "OnGround": false,
//!     "PortalCooldown": 0,
//!     "Pos": [-0.5, 0., 1.5],
//!     "Rotation": [-107.68715_f32, 0_f32],
//!     "UUID": [I; -411044392, 312166398, -1883713137, 1472542727],
//! });
//! assert_eq!(fastnbt::to_value(&axolotl), Ok(axolotl_nbt));
//! # }
//! # #[cfg(all(feature = "latest", feature = "serde", feature = "entities"))]
//! # test();
//! ```

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
