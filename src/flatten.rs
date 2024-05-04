// TODO: these things should arguably use serde_value::Value instead of fastnbt::Value

use std::{borrow::Cow, collections::HashMap, marker::PhantomData};

use serde::{
    de::{DeserializeSeed, Error, MapAccess, Visitor},
    Deserializer,
};

#[allow(unused)]
pub(crate) fn flatten(entity: &impl Flatten) -> HashMap<Cow<'static, str>, fastnbt::Value> {
    let mut map = HashMap::new();
    entity.flatten(&mut map);
    map
}

pub(crate) trait Flatten {
    fn flatten(&self, map: &mut HashMap<Cow<'static, str>, fastnbt::Value>);
}

impl<V> Flatten for HashMap<String, V>
where
    V: serde::Serialize,
{
    fn flatten(&self, map: &mut HashMap<Cow<'static, str>, fastnbt::Value>) {
        for (k, v) in self {
            map.insert(k.clone().into(), fastnbt::to_value(v).unwrap());
        }
    }
}

pub(crate) struct FlatMapDeserializer<'a, E>(
    pub(crate) &'a mut Vec<(fastnbt::Value, fastnbt::Value)>,
    pub(crate) PhantomData<E>,
);

impl<'a, E: Error> FlatMapDeserializer<'a, E> {
    fn deserialize_other<V>() -> Result<V, E> {
        Err(Error::custom("can only flatten structs and maps"))
    }
}

macro_rules! forward_to_deserialize_other {
    ($($func:ident ($($arg:ty),*))*) => {
        $(
            fn $func<V: Visitor<'de>>(self, $(_: $arg,)* _visitor: V) -> Result<V::Value, Self::Error> {
                Self::deserialize_other()
            }
        )*
    }
}

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

struct FlatMapAccess<'a, E> {
    iter: std::slice::Iter<'a, (fastnbt::Value, fastnbt::Value)>,
    pending_content: Option<&'a fastnbt::Value>,
    _marker: PhantomData<E>,
}

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
