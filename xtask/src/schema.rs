use std::{borrow::Cow, collections::BTreeMap};

use serde::{Deserialize, Serialize};

pub type FeaturesJson = Vec<Feature>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub name: String,
    pub mc: String,
    pub extractor: u8,
}

///////////////////////////////////////////////

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocksJson {
    pub blocks: Vec<Block>,
    pub enums: Vec<Enum>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub id: String,
    #[serde(default)]
    pub experimental: bool,
    pub properties: Vec<Property>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Property {
    Bool {
        name: String,
    },
    Int {
        name: String,
        min: u8,
        max: u8,
    },
    Enum {
        name: String,
        #[serde(rename = "enum")]
        enum_name: String,
    },
}

impl Property {
    pub fn name(&self) -> &str {
        match self {
            Property::Bool { name } => name,
            Property::Int { name, .. } => name,
            Property::Enum { name, .. } => name,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Enum {
    pub name: String,
    pub values: Vec<String>,
}

///////////////////////////////////////////////

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntitiesJson {
    pub entities: Vec<Entity>,
    pub types: Vec<EntityType>,
    pub compound_types: Vec<CompoundType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default)]
    pub experimental: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityType {
    pub name: String,
    pub parent: Option<String>,
    pub nbt: NbtCompound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NbtCompound {
    pub entries: BTreeMap<String, NbtCompoundEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompoundType {
    pub name: String,
    #[serde(flatten)]
    pub compound: NbtCompound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NbtCompoundEntry {
    pub value: NbtElement,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NbtElement {
    Any,
    Byte,
    Short,
    Int,
    Long,
    Float,
    Double,
    String,
    ByteArray,
    IntArray,
    LongArray,
    Uuid,
    Boolean,
    List { inner: Box<NbtElement> },
    AnyCompound,
    Compound { name: String },
}

impl NbtElement {
    pub fn as_rust_type(&self) -> Cow<'static, str> {
        match self {
            NbtElement::Any => "fastnbt::Value".into(),
            // TODO: use i8?
            NbtElement::Byte => "u8".into(),
            NbtElement::Short => "i16".into(),
            NbtElement::Int => "i32".into(),
            NbtElement::Long => "i64".into(),
            NbtElement::Float => "f32".into(),
            NbtElement::Double => "f64".into(),
            // TODO: try to use Cow<'a, str>?
            NbtElement::String => "String".into(),
            NbtElement::ByteArray => "fastnbt::ByteArray".into(),
            NbtElement::IntArray => "fastnbt::IntArray".into(),
            NbtElement::LongArray => "fastnbt::LongArray".into(),
            NbtElement::Uuid => "u128".into(),
            NbtElement::Boolean => "bool".into(),
            NbtElement::List { inner } => format!("Vec<{}>", inner.as_rust_type()).into(),
            NbtElement::AnyCompound => "std::collections::HashMap<String, fastnbt::Value>".into(),
            NbtElement::Compound { name } => format!("super::compounds::{name}").into(),
        }
    }
}