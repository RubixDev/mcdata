macro_rules! entities {
    (
        $mc_version:literal, $mc_mod:ident;
        $(
            $($experimental:ident)?
            $id:literal,
            $variant:ident:
            $type:ident
        );+
        $(;)?
    ) => {
        #[cfg(feature = "serde")]
        use std::{collections::HashMap, marker::PhantomData};
        #[cfg(feature = "serde")]
        use serde::{Deserialize, de::Visitor, Serialize};

        #[cfg(feature = "block-states")]
        pub(crate) type BlockState = $crate::$mc_mod::BlockState;
        #[cfg(not(feature = "block-states"))]
        pub(crate) type BlockState = $crate::GenericBlockState;

        #[allow(dead_code)]
        type CowStr = std::borrow::Cow<'static, str>;

        #[doc = concat!("A typed entity for Minecraft ", $mc_version, ".")]
        #[derive(Debug, Clone)]
        pub enum Entity {
            $(
                #[doc = concat!("`", $id, "`", $(" (", stringify!($experimental), ")")?)]
                #[allow(missing_docs)]
                $variant(types::$type),
            )+
            /// Any other unrecognized (possibly invalid) entity.
            Other(super::super::GenericEntity),
        }

        impl super::super::Entity for Entity {}

        #[cfg(feature = "serde")]
        impl Entity {
            /// Turn this entity into a
            /// [`GenericEntity`](super::super::GenericEntity).
            ///
            /// This internally allocates new strings. It is used for implementing equality, as the
            /// same entity can be represented by both a known variant and the [`Self::Other`]
            /// variant.
            pub fn as_generic(&self) -> super::super::GenericEntity {
                match self {
                    $(
                        Self::$variant(value) => {
                            let mut props = $crate::flatten::flatten(value);
                            let uuid: u128 = fastnbt::from_value(&props.remove("UUID").expect("every entity has a UUID")).expect("UUID from flattening should be valid");
                            super::super::GenericEntity {
                                id: $id.into(),
                                uuid,
                                properties: props,
                            }
                        }
                    )+
                    Self::Other(generic) => generic.clone(),
                }
            }
        }

        #[cfg(feature = "serde")]
        impl PartialEq for Entity {
            fn eq(&self, other: &Self) -> bool {
                self.as_generic() == other.as_generic()
            }
        }

        #[cfg(feature = "serde")]
        impl Serialize for Entity {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                self.as_generic().serialize(serializer)
            }
        }

        #[cfg(feature = "serde")]
        impl<'de> Deserialize<'de> for Entity {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct _Visitor<'de> {
                    marker: PhantomData<Entity>,
                    lifetime: PhantomData<&'de ()>,
                }
                impl<'de> Visitor<'de> for _Visitor<'de> {
                    type Value = Entity;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        formatter.write_str("Entity")
                    }

                    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
                    where
                        A: serde::de::MapAccess<'de>,
                    {
                        let mut id: Option<String> = None;
                        let mut uuid: Option<u128> = None;
                        let mut properties: HashMap<String, fastnbt::Value> = HashMap::new();
                        while let Some(key) = map.next_key::<String>()? {
                            match key.as_str() {
                                "id" => {
                                    if id.is_some() {
                                        return Err(serde::de::Error::duplicate_field("id"));
                                    }
                                    id = Some(map.next_value()?);
                                },
                                "UUID" => {
                                    if uuid.is_some() {
                                        return Err(serde::de::Error::duplicate_field("UUID"));
                                    }
                                    uuid = Some(map.next_value()?);
                                },
                                _ => {
                                    properties.insert(key, map.next_value()?);
                                },
                            }
                        }
                        let id = id.ok_or_else(|| serde::de::Error::missing_field("id"))?;
                        let uuid = uuid.ok_or_else(|| serde::de::Error::missing_field("UUID"))?;
                        Ok(match id.as_ref() {
                            $(
                                $id => {
                                    properties.insert("UUID".to_string(), fastnbt::to_value(uuid).expect("failed to serialize UUID"));
                                    match fastnbt::from_value::<types::$type>(&fastnbt::Value::Compound(properties.clone())) {
                                        Ok(val) => Self::Value::$variant(val),
                                        Err(_) => {
                                            properties.remove("UUID");
                                            Self::Value::Other(super::super::GenericEntity {
                                                id: $id.into(),
                                                uuid,
                                                properties: properties.into_iter().map(|(k, v)| (k.into(), v)).collect(),
                                            })
                                        }
                                    }
                                }
                            )+
                            _ => Self::Value::Other(super::super::GenericEntity {
                                id: id.into(),
                                uuid,
                                properties: properties.into_iter().map(|(k, v)| (k.into(), v)).collect(),
                            })
                        })
                    }
                }

                deserializer.deserialize_map(_Visitor {
                    marker: PhantomData::<Entity>,
                    lifetime: PhantomData,
                })
            }
        }
    };
}

macro_rules! entity_types {
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
        entity_types!(
            @impl
            concat!(
                "Entity types for Minecraft ", $mc_version, ".", r###"

The structs in this module represent the various superclasses of `Entity`, including those that
don't have a corresponding EntityType specified. Each of them can add additional data to the NBT
which all its subclasses will also have. In order to replicate this inheritance structure, every
struct in this module has a `parent` field which holds an instance of the struct that represents
the superclass. They all eventually go down to [`Entity`], which is the only struct wihout a
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
                    pub $entry_field: entity_types!(@optional $type $(, $optional)?),
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
                        entity_types!(@optional_insert map, $entry_name, &self.$entry_field $(, $optional)?);
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
                                let $entry_field = entity_types!(@missing $entry_field, $entry_name $(, $optional)?);
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
            entity_types!(@optional_insert $map, $entry_name, value);
        }
    };
}

macro_rules! entity_compound_types {
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
        entity_types!(
            @impl
            concat!("Additional typed NBT compounds for entities in Minecraft ", $mc_version, "."), compounds;
            $( $name, $(extra $extras_type)?, [$($($flat_field: $flat_type),*)?] { $( $($optional)? $entry_name as $entry_field : $type ),* } )*
        );
    };
}
