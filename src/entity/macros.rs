macro_rules! entities {
    (
        $mc_version:literal;
        $(
            $($experimental:ident)?
            $id:literal,
            $variant:ident:
            $type:ident
        );+
        $(;)?
    ) => {
        use std::{borrow::Cow, collections::HashMap, marker::PhantomData};
        #[cfg(feature = "serde")]
        use serde::{Deserialize, de::Visitor, Serialize};

        #[doc = concat!("A typed entity for Minecraft ", $mc_version, ".")]
        #[derive(Debug, Clone)]
        pub enum Entity<'a> {
            $(
                #[doc = concat!("`", $id, "`", $(" (", stringify!($experimental), ")")?)]
                #[allow(missing_docs)]
                $variant(types::$type),
            )+
            /// Any other unrecognized (possibly invalid) entity.
            Other(super::super::GenericEntity<'a>),
        }

        impl super::super::Entity for Entity<'_> {}

        impl Entity<'_> {
            /// Turn this entity into a
            /// [`GenericEntity`](super::super::GenericEntity).
            ///
            /// This internally allocates new strings. It is used for implementing equality, as the
            /// same entity can be represented by both a known variant and the [`Self::Other`]
            /// variant.
            pub fn as_generic(&self) -> super::super::GenericEntity<'_> {
                match self {
                    $(
                        Self::$variant(value) => {
                            let mut props = $crate::util::flatten(value);
                            let uuid: u128 = fastnbt::from_value(&props.remove("UUID").expect("every entity has a UUID")).expect("UUID from flattening should be valid");
                            super::super::GenericEntity {
                                id: Cow::Borrowed($id),
                                uuid,
                                properties: HashMap::from_iter(
                                    props.into_iter().map(|(k, v)| (Cow::Borrowed(k), v))
                                ),
                            }
                        }
                    )+
                    Self::Other(generic) => generic.clone(),
                }
            }
        }

        impl PartialEq for Entity<'_> {
            fn eq(&self, other: &Self) -> bool {
                self.as_generic() == other.as_generic()
            }
        }

        #[cfg(feature = "serde")]
        impl Serialize for Entity<'_> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                self.as_generic().serialize(serializer)
            }
        }

        #[cfg(feature = "serde")]
        impl<'de: 'a, 'a> Deserialize<'de> for Entity<'a> {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct _Visitor<'de: 'a, 'a> {
                    marker: PhantomData<Entity<'a>>,
                    lifetime: PhantomData<&'de ()>,
                }
                impl<'de: 'a, 'a> Visitor<'de> for _Visitor<'de, 'a> {
                    type Value = Entity<'a>;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        formatter.write_str("Entity")
                    }

                    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
                    where
                        A: serde::de::MapAccess<'de>,
                    {
                        let mut id: Option<Cow<'a, str>> = None;
                        let mut uuid: Option<u128> = None;
                        let mut properties: HashMap<Cow<'a, str>, fastnbt::Value> = HashMap::new();
                        while let Some(key) = map.next_key::<Cow<'a, str>>()? {
                            match key.as_ref() {
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
                                    properties.insert(Cow::Borrowed("UUID"), fastnbt::to_value(uuid).expect("failed to serialize UUID"));
                                    match fastnbt::from_value::<types::$type>(&fastnbt::to_value(&properties).expect("`HashMap<Cow<str>, Value>` can be turned into `Value::Compound`")) {
                                        Ok(val) => Self::Value::$variant(val),
                                        Err(_) => {
                                            properties.remove("UUID");
                                            Self::Value::Other(super::super::GenericEntity {
                                                id: Cow::Borrowed($id),
                                                uuid,
                                                properties,
                                            })
                                        }
                                    }
                                }
                            )+
                            _ => Self::Value::Other(super::super::GenericEntity { id, uuid, properties })
                        })
                    }
                }

                deserializer.deserialize_map(_Visitor {
                    marker: PhantomData::<Entity<'a>>,
                    lifetime: PhantomData,
                })
            }
        }
    };
}

macro_rules! entity_types {
    ($mc_version:literal; $($tt:tt)*) => {
        entity_types!(
            @impl
            concat!(
                "Entity types for Minecraft ", $mc_version, ".", r###"

The structs in this module represent the various superclasses of `Entity`, including those that
don't have a corresponding EntityType specified. Each of them can add additional data to the NBT
which all its subclasses will also have. In order to replicate this inheritence structure, every
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
            $($tt)*
        );
    };
    (
        @impl
        $doc:expr, $mod_name:ident;
        $(
            $name:ident
            $( > $parent:ident)?
            - $(
                $($optional:ident)?
                $entry_name:literal as $entry_field:ident
                : $type:ty
            ),*
        );*
        $(;)?
    ) => {
        #[doc = $doc]
        #[allow(missing_docs, unused_imports)]
        pub mod $mod_name {
            use $crate::util::Flatten;
            #[cfg(feature = "serde")]
            use serde::{Deserialize, de::Visitor, Serialize};
            #[cfg(feature = "serde")]
            use std::{marker::PhantomData, fmt};

            $(
            #[derive(Debug, Clone, PartialEq)]
            pub struct $name {
                $(
                    #[doc = concat!("`\"", $entry_name, "\"`")]
                    pub $entry_field: entity_types!(@optional $type $(, $optional)?),
                )*
                $(
                    pub parent: $parent,
                )?
            }

            impl Flatten for $name {
                fn flatten(&self, map: &mut std::collections::HashMap<&'static str, fastnbt::Value>) {
                    $(
                        map.insert($entry_name, fastnbt::value::to_value(&self.$entry_field).expect("structure is valid NBT"));
                    )*
                    $(
                        <$parent as Flatten>::flatten(&self.parent, map);
                    )?
                }
            }

            #[cfg(feature = "serde")]
            impl Serialize for $name {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: serde::Serializer,
                {
                    $crate::util::flatten(self).serialize(serializer)
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
                                    parent: <$parent as Deserialize>::deserialize($crate::util::FlatMapDeserializer(&mut __collect, PhantomData))?,
                                )?
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
}

macro_rules! entity_compound_types {
    (
        $mc_version:literal;
        $(
            $name:ident
            - $(
                $($optional:ident)?
                $entry_name:literal as $entry_field:ident
                : $type:ty
            ),*
        );*
        $(;)?
    ) => {
        entity_types!(
            @impl
            concat!("Additional typed NBT compounds for entities in Minecraft ", $mc_version, "."), compounds;
            $( $name - $( $($optional)? $entry_name as $entry_field : $type ),* );*
        );
    };
}
