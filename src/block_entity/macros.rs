macro_rules! block_entities {
    (
        $mc_version:literal, $mc_mod:ident;
        $(
            $id:literal,
            $variant:ident:
            $type:ident
            ($($parent:tt)+),
            $fields:expr
        );+
        $(;)?
    ) => {
        #[cfg(feature = "serde")]
        use std::{collections::HashMap, marker::PhantomData};
        #[cfg(feature = "serde")]
        use serde::{Deserialize, de::Visitor, Serialize};

        #[cfg(feature = "block-states")]
        pub(crate) type BlockState = $crate::block_state::$mc_mod::BlockState;
        #[cfg(not(feature = "block-states"))]
        pub(crate) type BlockState = $crate::block_state::GenericBlockState;

        #[allow(dead_code)]
        type CowStr = std::borrow::Cow<'static, str>;

        #[doc = concat!("A typed block entity for Minecraft ", $mc_version, ".")]
        #[derive(Debug, Clone)]
        pub enum BlockEntity {
            $(
                #[doc = concat!("`", $id, "`")]
                #[allow(missing_docs)]
                $variant(types::$type),
            )+
            /// Any other unrecognized (possibly invalid) block entity.
            Other(super::super::GenericBlockEntity),
        }

        impl super::super::BlockEntity for BlockEntity {
            fn position(&self) -> $crate::util::BlockPos {
                match self {
                    $(
                        Self::$variant(t) => $crate::util::BlockPos::new(
                            block_entities!(@parent_block_entity t $($parent)+).x,
                            block_entities!(@parent_block_entity t $($parent)+).y,
                            block_entities!(@parent_block_entity t $($parent)+).z,
                        ),
                    )+
                    Self::Other(generic) => generic.pos,
                }
            }
        }

        #[cfg(feature = "serde")]
        impl BlockEntity {
            /// Turn this entity into a
            /// [`GenericBlockEntity`](super::super::GenericBlockEntity).
            ///
            /// This internally allocates new strings. It is used for implementing equality, as the
            /// same block entity can be represented by both a known variant and the [`Self::Other`]
            /// variant.
            pub fn as_generic(&self) -> super::super::GenericBlockEntity {
                match self {
                    $(
                        Self::$variant(value) => {
                            let mut props = $crate::flatten::flatten(value);
                            let Some(fastnbt::Value::Int(x)) = props.remove("x") else {
                                panic!("valid block entity has 'x' key of type int");
                            };
                            let Some(fastnbt::Value::Int(y)) = props.remove("y") else {
                                panic!("valid block entity has 'y' key of type int");
                            };
                            let Some(fastnbt::Value::Int(z)) = props.remove("z") else {
                                panic!("valid block entity has 'z' key of type int");
                            };
                            super::super::GenericBlockEntity {
                                id: $id.into(),
                                pos: $crate::util::BlockPos::new(x, y, z),
                                properties: props,
                            }
                        }
                    )+
                    Self::Other(generic) => generic.clone(),
                }
            }
        }

        #[cfg(feature = "serde")]
        impl PartialEq for BlockEntity {
            fn eq(&self, other: &Self) -> bool {
                self.as_generic() == other.as_generic()
            }
        }

        #[cfg(feature = "serde")]
        impl Serialize for BlockEntity {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                self.as_generic().serialize(serializer)
            }
        }

        #[cfg(feature = "serde")]
        impl<'de> Deserialize<'de> for BlockEntity {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct _Visitor<'de> {
                    marker: PhantomData<BlockEntity>,
                    lifetime: PhantomData<&'de ()>,
                }
                impl<'de> Visitor<'de> for _Visitor<'de> {
                    type Value = BlockEntity;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        formatter.write_str("Entity")
                    }

                    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
                    where
                        A: serde::de::MapAccess<'de>,
                    {
                        let mut id: Option<String> = None;
                        let mut x: Option<i32> = None;
                        let mut y: Option<i32> = None;
                        let mut z: Option<i32> = None;
                        let mut properties: HashMap<String, fastnbt::Value> = HashMap::new();
                        while let Some(key) = map.next_key::<String>()? {
                            match key.as_str() {
                                "id" => {
                                    if id.is_some() {
                                        return Err(serde::de::Error::duplicate_field("id"));
                                    }
                                    id = Some(map.next_value()?);
                                },
                                "x" => {
                                    if x.is_some() {
                                        return Err(serde::de::Error::duplicate_field("x"));
                                    }
                                    x = Some(map.next_value()?);
                                },
                                "y" => {
                                    if y.is_some() {
                                        return Err(serde::de::Error::duplicate_field("y"));
                                    }
                                    y = Some(map.next_value()?);
                                },
                                "z" => {
                                    if z.is_some() {
                                        return Err(serde::de::Error::duplicate_field("z"));
                                    }
                                    z = Some(map.next_value()?);
                                },
                                _ => {
                                    properties.insert(key, map.next_value()?);
                                },
                            }
                        }
                        let x = x.ok_or_else(|| serde::de::Error::missing_field("x"))?;
                        let y = y.ok_or_else(|| serde::de::Error::missing_field("y"))?;
                        let z = z.ok_or_else(|| serde::de::Error::missing_field("z"))?;
                        Ok(match id.as_deref() {
                            $(
                                Some($id) => {
                                    properties.insert("x".to_string(), fastnbt::Value::Int(x));
                                    properties.insert("y".to_string(), fastnbt::Value::Int(y));
                                    properties.insert("z".to_string(), fastnbt::Value::Int(z));
                                    match fastnbt::from_value::<types::$type>(&fastnbt::Value::Compound(properties.clone())) {
                                        Ok(val) => Self::Value::$variant(val),
                                        Err(_) => {
                                            properties.remove("x");
                                            properties.remove("y");
                                            properties.remove("z");
                                            Self::Value::Other(super::super::GenericBlockEntity {
                                                id: $id.into(),
                                                pos: $crate::util::BlockPos::new(x, y, z),
                                                properties: properties.into_iter().map(|(k, v)| (k.into(), v)).collect(),
                                            })
                                        }
                                    }
                                }
                            )+
                            Some(id) => Self::Value::Other(super::super::GenericBlockEntity {
                                id: id.to_string().into(),
                                pos: $crate::util::BlockPos::new(x, y, z),
                                properties: properties.into_iter().map(|(k, v)| (k.into(), v)).collect(),
                            }),
                            None => {
                                // try untagged deserialization when id is missing
                                properties.insert("x".to_string(), fastnbt::Value::Int(x));
                                properties.insert("y".to_string(), fastnbt::Value::Int(y));
                                properties.insert("z".to_string(), fastnbt::Value::Int(z));
                                // for untagged deserialization we never want to discard data in the
                                // properties. We do that by first calculating how many keys in the
                                // properties are not present in each variant and then trying to
                                // deserialize each one where that count is 0, in order.
                                // This still means that untagged chests will be deserialized as
                                // barrels, but there's nothing we can do about that.
                                let variants = [$(
                                    (
                                        // the number of keys in the properties that this variant
                                        // doesn't contain. the `- 3` is for the x, y, and z keys.
                                        properties.keys().filter(|k| !$fields.contains(&k.as_str())).count() - 3,
                                        stringify!($variant),
                                        // a closure to try to construct this variant
                                        Box::new(
                                            || fastnbt::from_value::<types::$type>(
                                                &fastnbt::Value::Compound(properties.clone())
                                            ).ok().map(Self::Value::$variant)
                                        ) as Box<dyn FnOnce() -> Option<Self::Value>>,
                                    ),
                                )+];
                                for (unused_keys, _, closure) in variants {
                                    // only accept variants which make use of all keys in the
                                    // properties
                                    if unused_keys > 0 {
                                        continue;
                                    }
                                    if let Some(ok) = closure() {
                                        return Ok(ok);
                                    }
                                }
                                properties.remove("x");
                                properties.remove("y");
                                properties.remove("z");
                                Self::Value::Other(super::super::GenericBlockEntity {
                                    id: "".into(),
                                    pos: $crate::util::BlockPos::new(x, y, z),
                                    properties: properties.into_iter().map(|(k, v)| (k.into(), v)).collect(),
                                })
                            }
                        })
                    }
                }

                deserializer.deserialize_map(_Visitor {
                    marker: PhantomData::<BlockEntity>,
                    lifetime: PhantomData,
                })
            }
        }
    };
    (@parent_block_entity $self:ident > BlockEntity) => { $self.parent };
    (@parent_block_entity $self:ident > $($rest:tt)+) => { block_entities!(@parent_block_entity $self $($rest)+).parent };
}

macro_rules! block_entity_types {
    (
        $mc_version:literal;
        $(
            $name:ident
            $( > $parent:ident)?
            $(, with extras as $extras_type:ty)?
            $(, flattened [$( $flat_field:ident : $flat_type:ty ),*])?
            { $(
                $($optional:ident)?
                $entry_name:literal as $entry_field:ident
                : $type:ty
            ),* }
        )*
    ) => {
        block_entity_types!(
            @impl
            concat!(
                "Block entity types for Minecraft ", $mc_version, ".", r###"

The structs in this module represent the various superclasses of `BlockEntity`, including those that
don't have a corresponding BlockEntityType specified. Each of them can add additional data to the NBT
which all its subclasses will also have. In order to replicate this inheritance structure, every
struct in this module has a `parent` field which holds an instance of the struct that represents
the superclass. They all eventually go down to [`BlockEntity`], which is the only struct wihout a
parent, as it is the base class of all the others. During (de)serialization this structure is
flattened to one level. This is best described with an example. Consider the following structure:

```
struct A { a: i32 }
struct B { b: f64, parent: A }
struct C { c: bool, parent: B }
```

During (de)serialization an instance of `C` would be treated as if it was defined as:

```
struct C { a: i32, b: f64, c: bool }
```

The same goes for `B` which would be seen as

```
struct B { a: i32, b: f64 }
```
"###
            ), types;
            $( $name $(> $parent)?, $(extra $extras_type)?, [$($($flat_field: $flat_type),*)?] { $( $($optional)? $entry_name as $entry_field : $type ),* } )*
        );
    };
    (
        @impl
        $doc:expr, $mod_name:ident;
        $(
            $name:ident
            $( > $parent:ident)?,
            $(extra $extras_type:ty)?,
            [$( $flat_field:ident : $flat_type:ty ),*]
            { $(
                $($optional:ident)?
                $entry_name:literal as $entry_field:ident
                : $type:ty
            ),* }
        )*
    ) => {
        #[doc = $doc]
        #[allow(missing_docs, unused_imports, non_camel_case_types)]
        pub mod $mod_name {
            use std::collections::HashMap;

            #[cfg(feature = "serde")]
            use $crate::flatten::Flatten;
            #[cfg(feature = "serde")]
            use serde::{Deserialize, de::Visitor, Serialize};
            #[cfg(feature = "serde")]
            use std::{marker::PhantomData, fmt};

            #[allow(dead_code)]
            type CowStr = std::borrow::Cow<'static, str>;

            $(
            #[derive(Debug, Clone)]
            #[cfg_attr(feature = "serde", derive(PartialEq))]
            pub struct $name {
                $(
                    #[doc = concat!("`\"", $entry_name, "\"`")]
                    pub $entry_field: block_entity_types!(@optional $type $(, $optional)?),
                )*
                $(
                    #[doc = concat!("Inherited fields from [`", stringify!($parent), "`]")]
                    pub parent: $parent,
                )?
                $(
                    /// Additional fields with unknown keys.
                    pub extra: HashMap<CowStr, $extras_type>,
                )?
                $(
                    // TODO: these doc links can be wrong for `Box<T>`
                    #[doc = concat!("Inherited fields from [`", stringify!($flat_type), "`]")]
                    pub $flat_field: $flat_type,
                )*
            }

            #[cfg(feature = "serde")]
            impl Flatten for $name {
                fn flatten(&self, map: &mut HashMap<CowStr, fastnbt::Value>) {
                    $(
                        block_entity_types!(@optional_insert map, $entry_name, &self.$entry_field $(, $optional)?);
                    )*
                    $(
                        <$parent as Flatten>::flatten(&self.parent, map);
                    )?
                    $(
                        stringify!($extras_type); // just to have the correct macro variable in here somewhere
                        for (k, v) in &self.extra {
                            map.insert(CowStr::Owned(k.clone()), fastnbt::to_value(v).expect("structure is valid NBT"));
                        }
                    )?
                    $(
                        self.$flat_field.flatten(map);
                    )*
                }
            }

            #[cfg(feature = "serde")]
            impl Serialize for $name {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: serde::Serializer,
                {
                    $crate::flatten::flatten(self).serialize(serializer)
                }
            }

            #[cfg(feature = "serde")]
            impl<'de> Deserialize<'de> for $name {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where D: serde::Deserializer<'de> {
                    #[allow(non_camel_case_types)]
                    enum _Field {
                        $($entry_field,)*
                        __Other(fastnbt::Value),
                    }
                    impl<'de> Deserialize<'de> for _Field {
                        fn deserialize<D>(deserializer: D) -> Result<_Field, D::Error>
                        where D: serde::Deserializer<'de> {
                            struct _FieldVisitor;

                            impl<'de> Visitor<'de> for _FieldVisitor {
                                type Value = _Field;

                                fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
                                    fmt.write_str("field identifier")
                                }

                                fn visit_str<E>(self, value: &str) -> Result<_Field, E>
                                where E: serde::de::Error {
                                    match value {
                                        $(
                                            $entry_name => Ok(_Field::$entry_field),
                                        )*
                                        _ => Ok(_Field::__Other(fastnbt::Value::String(value.to_string()))),
                                    }
                                }
                            }

                            deserializer.deserialize_identifier(_FieldVisitor)
                        }
                    }

                    struct _Visitor<'de> {
                        marker: PhantomData<$name>,
                        lifetime: PhantomData<&'de ()>,
                    }
                    impl<'de> Visitor<'de> for _Visitor<'de> {
                        type Value = $name;

                        fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
                            fmt.write_str(concat!("struct ", stringify!($name)))
                        }

                        fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
                        where V: serde::de::MapAccess<'de>
                        {
                            $(
                                let mut $entry_field: Option<$type> = None;
                            )*
                            let mut __collect = Vec::<(fastnbt::Value, fastnbt::Value)>::new();
                            while let Some(key) = map.next_key()? {
                                match key {
                                    $(
                                        _Field::$entry_field => {
                                            if $entry_field.is_some() {
                                                return Err(serde::de::Error::duplicate_field($entry_name));
                                            }
                                            $entry_field = Some(map.next_value()?);
                                        }
                                    )*
                                    _Field::__Other(name) => {
                                        __collect.push((name, map.next_value()?));
                                    }
                                }
                            }
                            $(
                                let $entry_field = block_entity_types!(@missing $entry_field, $entry_name $(, $optional)?);
                            )*
                            Ok($name {
                                $($entry_field,)*
                                $(
                                    parent: <$parent as Deserialize>::deserialize($crate::flatten::FlatMapDeserializer(&mut __collect, PhantomData))?,
                                )?
                                $(
                                    extra: <HashMap<CowStr, $extras_type> as Deserialize>::deserialize($crate::flatten::FlatMapDeserializer(&mut __collect, PhantomData))?,
                                )?
                                $(
                                    $flat_field: <$flat_type as Deserialize>::deserialize($crate::flatten::FlatMapDeserializer(&mut __collect, PhantomData))?,
                                )*
                            })
                        }
                    }

                    deserializer.deserialize_map(_Visitor {
                        marker: PhantomData::<$name>,
                        lifetime: PhantomData,
                    })
                }
            }
        )*}
    };
    (@optional $type:ty) => { $type };
    (@optional $type:ty, $optional:ident) => { Option<$type> };
    (@missing $entry_field:ident, $entry_name:literal) => { $entry_field.ok_or_else(|| serde::de::Error::missing_field($entry_name))? };
    (@missing $entry_field:ident, $entry_name:literal, $optional:ident) => { $entry_field };
    (@optional_insert $map:ident, $entry_name:literal, $entry_value:expr) => {
        $map.insert(CowStr::Borrowed($entry_name), fastnbt::to_value($entry_value).expect("structure is valid NBT"));
    };
    (@optional_insert $map:ident, $entry_name:literal, $entry_value:expr, $optional:ident) => {
        if let Some(value) = $entry_value {
            block_entity_types!(@optional_insert $map, $entry_name, value);
        }
    };
}

macro_rules! block_entity_compound_types {
    (
        $mc_version:literal;
        $(
            $name:ident
            $(, with extras as $extras_type:ty)?
            $(, flattened [$( $flat_field:ident : $flat_type:ty ),*])?
            { $(
                $($optional:ident)?
                $entry_name:literal as $entry_field:ident
                : $type:ty
            ),* }
        )*
    ) => {
        block_entity_types!(
            @impl
            concat!("Additional typed NBT compounds for block entities in Minecraft ", $mc_version, "."), compounds;
            $( $name, $(extra $extras_type)?, [$($($flat_field: $flat_type),*)?] { $( $($optional)? $entry_name as $entry_field : $type ),* } )*
        );
    };
}
