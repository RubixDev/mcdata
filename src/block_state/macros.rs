#[allow(unused_macros)] // is only used when any mc version feature is enabled
macro_rules! prop_enums {
    ($mc_version:literal; $($name:ident => $($variant:ident),+);+ $(;)?) => {
        #[doc = concat!("Property types for Minecraft ", $mc_version, ".")]
        #[allow(missing_docs)]
        pub mod props {$(
            #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumString)]
            #[strum(serialize_all = "snake_case")]
            #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
            #[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
            pub enum $name { $($variant),+ }
        )+}
    };
}

macro_rules! blocks {
    (
        $mc_version:literal;
        $(
            $($experimental:ident)?
            $id:literal,
            $variant:ident,
            $map_color:ident
            $(-
                $($prop:ident : $type:ty $(as $prop_str:literal)?),+
            )?
        );+
        $(;)?
    ) => {
        use std::collections::HashMap;

        #[cfg(feature = "serde")]
        use std::{fmt, marker::PhantomData, str::FromStr};
        #[cfg(feature = "serde")]
        use serde::{Deserialize, de::Visitor, Serialize};

        #[cfg(feature = "serde")]
        type CowStr = std::borrow::Cow<'static, str>;

        #[doc = concat!("A typed block state for Minecraft ", $mc_version, ".")]
        #[derive(Debug, Clone)]
        pub enum BlockState {
            $(
                #[doc = concat!("`", $id, "`", $(" (", stringify!($experimental), ")")?)]
                #[allow(missing_docs)]
                $variant $({ $($prop: $type),+ })?,
            )+
            /// Any other unrecognized (possibly invalid) block state with a name and properties as
            /// strings.
            Other(super::super::GenericBlockState),
        }

        impl super::super::BlockState for BlockState {
            fn air() -> Self {
                Self::Air
            }
        }

        impl BlockState {
            /// Turn this block state into a
            /// [`GenericBlockState`](super::super::GenericBlockState).
            ///
            /// This internally allocates new strings. It is used for implementing equality, as the
            /// same block state can be represented by both a known variant and the [`Self::Other`]
            /// variant.
            pub fn as_generic(&self) -> super::super::GenericBlockState {
                match self {
                    $(
                        Self::$variant $({ $($prop),+ })? => super::super::GenericBlockState {
                            name: $id.into(),
                            properties: props!($( $($prop $($prop_str)?),+ )?),
                        },
                    )+
                    Self::Other(generic) => generic.clone(),
                }
            }

            /// Get the color this block's default state has on maps.
            ///
            /// Note: Every block state represented by the [`Self::Other`] variant will use
            /// [`MapColor::None`].
            pub fn map_color(&self) -> MapColor {
                match self {
                    $(
                        Self::$variant $({ $($prop: _),+ })? => MapColor::$map_color,
                    )+
                    Self::Other(_) => MapColor::None,
                }
            }
        }

        impl PartialEq for BlockState {
            fn eq(&self, other: &Self) -> bool {
                self.as_generic() == other.as_generic()
            }
        }
        impl Eq for BlockState {}

        #[cfg(feature = "serde")]
        impl Serialize for BlockState {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                self.as_generic().serialize(serializer)
            }
        }

        #[cfg(feature = "serde")]
        impl<'de> Deserialize<'de> for BlockState {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where D: serde::Deserializer<'de> {
                const FIELDS: &[&str] = &["Name", "Properties"];
                enum _Field { Name, Properties }
                impl<'de> Deserialize<'de> for _Field {
                    fn deserialize<D>(deserializer: D) -> Result<_Field, D::Error>
                    where D: serde::Deserializer<'de> {
                        struct _FieldVisitor;

                        impl<'de> Visitor<'de> for _FieldVisitor {
                            type Value = _Field;

                            fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
                                fmt.write_str("`Name` or `Properties`")
                            }

                            fn visit_str<E>(self, value: &str) -> Result<_Field, E>
                            where E: serde::de::Error {
                                match value {
                                    "Name" => Ok(_Field::Name),
                                    "Properties" => Ok(_Field::Properties),
                                    _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                                }
                            }
                        }

                        deserializer.deserialize_identifier(_FieldVisitor)
                    }
                }

                struct _Visitor<'de> {
                    marker: PhantomData<BlockState>,
                    lifetime: PhantomData<&'de ()>,
                }
                impl<'de> Visitor<'de> for _Visitor<'de> {
                    type Value = BlockState;

                    fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
                        fmt.write_str("BlockState")
                    }

                    fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
                    where V: serde::de::MapAccess<'de>
                    {
                        let mut name: Option<CowStr> = None;
                        let mut properties: Option<HashMap<CowStr, CowStr>> = None;
                        while let Some(key) = map.next_key()? {
                            match key {
                                _Field::Name => {
                                    if name.is_some() {
                                        return Err(serde::de::Error::duplicate_field("name"));
                                    }
                                    name = Some(map.next_value()?);
                                }
                                _Field::Properties => {
                                    if properties.is_some() {
                                        return Err(serde::de::Error::duplicate_field("properties"));
                                    }
                                    properties = Some(map.next_value()?);
                                }
                            }
                        }
                        let name = name.ok_or_else(|| serde::de::Error::missing_field("name"))?;
                        let properties = properties.unwrap_or_default();
                        Ok(match name.as_ref() {
                            $(
                                $id => blocks!(@block $variant $(, name, properties; $($prop $($prop_str)?),+)?)
                            ),+,
                            _ => Self::Value::Other(super::super::GenericBlockState { name, properties }),
                        })
                    }
                }

                deserializer.deserialize_struct("BlockState", FIELDS, _Visitor {
                    marker: PhantomData::<BlockState>,
                    lifetime: PhantomData,
                })
            }
        }
    };
    (@block $block:ident) => { Self::Value::$block };
    (@block $block:ident, $name:ident, $props:ident; $( $prop:ident $($prop_str:literal)? ),+) => {
        Self::Value::$block {
            $(
                $prop: match $props.get(prop_str!($prop $($prop_str)?)).and_then(|val| <_>::from_str(val).ok()) {
                    Some(val) => val,
                    None => return Ok(Self::Value::Other(super::super::GenericBlockState { name: $name, properties: $props })),
                }
            ),+
        }
    };
}

macro_rules! props {
    () => { HashMap::new() };
    ($($prop:ident $($prop_str:literal)?),+) => {
        HashMap::from([$(
            (prop_str!($prop $($prop_str)?).into(), $prop.to_string().into()),
        )+])
    }
}

macro_rules! prop_str {
    ($prop:ident $prop_str:literal) => {
        $prop_str
    };
    ($prop:ident) => {
        stringify!($prop)
    };
}

macro_rules! map_colors {
    ($mc_version:literal; $( $name:ident => $color:literal $rgb:tt ),+ $(,)?) => {
        /// The possible colors on Minecraft maps.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(u32)]
        pub enum MapColor {
            /// No map color, fully transparent.
            None = 0,
            $(
                #[doc = concat!(r#"<div style="display: inline-block; width: 3em; height: 1em; border: 1px solid black; background: rgb"#, stringify!($rgb), r#";"></div>"#)]
                $name = $color,
            )+
        }

        impl MapColor {
            /// Returns a packed ARGB representation of this color as a [`u32`].
            ///
            /// The alpha channel will always be `0xff`, except for [`MapColor::None`].
            #[inline]
            pub const fn packed_argb(&self) -> u32 {
                match self {
                    Self::None => 0,
                    _ => 0xff000000 | *self as u32,
                }
            }

            /// Returns an RGBA array of this color.
            ///
            /// The alpha channel will always be `0xff`, except for [`MapColor::None`].
            #[inline]
            pub const fn rgba(&self) -> [u8; 4] {
                self.packed_argb().rotate_left(8).to_be_bytes()
            }

            /// Calculate the RGBA color for this map color with the given brightness.
            ///
            /// The alpha channel will always be `0xff`, except for [`MapColor::None`].
            pub const fn calc_rgba(&self, brightness: u8) -> [u8; 4] {
                let [r, g, b, a] = self.rgba();
                let brightness = brightness as u32;
                [
                    ((r as u32) * brightness / 255) as u8,
                    ((g as u32) * brightness / 255) as u8,
                    ((b as u32) * brightness / 255) as u8,
                    a
                ]
            }
        }
    };
}
