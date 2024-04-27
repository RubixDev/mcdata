use serde::{Deserialize, Serialize};

pub type FeaturesJson = Vec<Feature>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub name: String,
    pub mc: String,
    pub extractor: u8,
}

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
