//! Other Minecraft related types.

use std::{borrow::Cow, collections::HashMap};

/// A Minecraft BlockPos storing an integer coordinate in 3D.
///
/// The [`x`](Self::x), [`y`](Self::y), and [`z`](Self::z) components are stored as [`i32`]s and
/// can thus be both positive and negative.
/// If only positive values should be allowed, use [`UVec3`] instead.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockPos {
    /// The `x` component of this vector.
    pub x: i32,
    /// The `y` component of this vector.
    pub y: i32,
    /// The `z` component of this vector.
    pub z: i32,
}

impl BlockPos {
    /// The position at (0, 0, 0).
    pub const ORIGIN: Self = Self::new(0, 0, 0);

    /// Create a new [`BlockPos`] given values for [`x`](Self::x), [`y`](Self::y), and
    /// [`z`](Self::z).
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Compute the total volume of the box containing [`ORIGIN`](Self::ORIGIN) and `self`.
    pub const fn volume(&self) -> usize {
        self.x.unsigned_abs() as usize
            * self.y.unsigned_abs() as usize
            * self.z.unsigned_abs() as usize
    }

    /// Convert this position into a [`UVec3`] with the [absolute values](i32::unsigned_abs) of
    /// each component.
    pub const fn abs(&self) -> UVec3 {
        UVec3 {
            x: self.x.unsigned_abs(),
            y: self.x.unsigned_abs(),
            z: self.x.unsigned_abs(),
        }
    }
}

/// A Positive integer coordinate in 3D.
///
/// The [`x`](Self::x), [`y`](Self::y), and [`z`](Self::z) components are stored as [`u32`]s and
/// can thus only be positive.
/// If negative values should also be allowed, use [`BlockPos`] instead.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UVec3 {
    /// The `x` component of this vector.
    pub x: u32,
    /// The `y` component of this vector.
    pub y: u32,
    /// The `z` component of this vector.
    pub z: u32,
}

impl UVec3 {
    /// The position at (0, 0, 0).
    pub const ORIGIN: Self = Self::new(0, 0, 0);

    /// Create a new [`UVec3`] given values for [`x`](Self::x), [`y`](Self::y), and
    /// [`z`](Self::z).
    pub const fn new(x: u32, y: u32, z: u32) -> Self {
        Self { x, y, z }
    }

    /// Compute the total volume of the box containing [`ORIGIN`](Self::ORIGIN) and `self`.
    pub const fn volume(&self) -> usize {
        self.x as usize * self.y as usize * self.z as usize
    }
}

macro_rules! vec_debug {
    ($type:ty) => {
        impl std::fmt::Debug for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "({}, {}, {})", self.x, self.y, self.z)
            }
        }
    };
}
vec_debug!(BlockPos);
vec_debug!(UVec3);

// TODO: things below should arguably use serde_value::Value instead of fastnbt::Value

#[allow(unused)]
pub(crate) fn flatten(entity: &impl Flatten) -> HashMap<Cow<'static, str>, fastnbt::Value> {
    let mut map = HashMap::new();
    entity.flatten(&mut map);
    map
}

pub(crate) trait Flatten {
    fn flatten(&self, map: &mut HashMap<Cow<'static, str>, fastnbt::Value>);
}

#[cfg(feature = "serde")]
use serde::{
    de::{DeserializeSeed, Error, MapAccess, Visitor},
    Deserializer,
};
#[cfg(feature = "serde")]
use std::marker::PhantomData;

#[cfg(feature = "serde")]
pub(crate) struct FlatMapDeserializer<'a, E>(
    pub(crate) &'a mut Vec<(fastnbt::Value, fastnbt::Value)>,
    pub(crate) PhantomData<E>,
);

#[cfg(feature = "serde")]
impl<'a, E: Error> FlatMapDeserializer<'a, E> {
    fn deserialize_other<V>() -> Result<V, E> {
        Err(Error::custom("can only flatten structs and maps"))
    }
}

#[cfg(feature = "serde")]
macro_rules! forward_to_deserialize_other {
    ($($func:ident ($($arg:ty),*))*) => {
        $(
            fn $func<V: Visitor<'de>>(self, $(_: $arg,)* _visitor: V) -> Result<V::Value, Self::Error> {
                Self::deserialize_other()
            }
        )*
    }
}

#[cfg(feature = "serde")]
impl<'a: 'de, 'de, E: Error> Deserializer<'de> for FlatMapDeserializer<'a, E> {
    type Error = E;

    forward_to_deserialize_other! {
        deserialize_bool()
        deserialize_i8()
        deserialize_i16()
        deserialize_i32()
        deserialize_i64()
        deserialize_u8()
        deserialize_u16()
        deserialize_u32()
        deserialize_u64()
        deserialize_f32()
        deserialize_f64()
        deserialize_char()
        deserialize_str()
        deserialize_string()
        deserialize_bytes()
        deserialize_byte_buf()
        deserialize_unit_struct(&'static str)
        deserialize_seq()
        deserialize_tuple(usize)
        deserialize_tuple_struct(&'static str, usize)
        deserialize_identifier()
        deserialize_option()
        deserialize_enum(&'static str, &'static [&'static str])
    }

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deserialize_map(visitor)
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_unit()
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_map(FlatMapAccess {
            iter: self.0.iter(),
            pending_content: None,
            _marker: PhantomData,
        })
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error> {
        todo!("do I need this?")
    }
}

#[cfg(feature = "serde")]
struct FlatMapAccess<'a, E> {
    iter: std::slice::Iter<'a, (fastnbt::Value, fastnbt::Value)>,
    pending_content: Option<&'a fastnbt::Value>,
    _marker: PhantomData<E>,
}

#[cfg(feature = "serde")]
impl<'a: 'de, 'de, E: Error> MapAccess<'de> for FlatMapAccess<'a, E> {
    type Error = E;

    fn next_key_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        if let Some((key, content)) = self.iter.next() {
            self.pending_content = Some(content);
            return seed
                .deserialize(key)
                .map(Some)
                .map_err(|e| Error::custom(e));
        }
        Ok(None)
    }

    fn next_value_seed<T>(&mut self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.pending_content.take() {
            Some(value) => seed.deserialize(value).map_err(|e| Error::custom(e)),
            None => Err(Error::custom("value is missing")),
        }
    }
}
