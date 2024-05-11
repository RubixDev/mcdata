#![doc = include_str!("../README.md")]
#![cfg_attr(
    feature = "docs",
    cfg_attr(doc, doc = ::document_features::document_features!(feature_label = r#"<span class="stab portability"><code>{feature}</code></span>"#))
)]
#![cfg_attr(all(doc, CHANNEL_NIGHTLY), feature(doc_auto_cfg))]
#![warn(missing_docs, rust_2018_idioms)]

mod block_entity;
mod block_state;
mod combined;
pub mod data_version;
mod entity;
pub mod util;

#[cfg(feature = "serde")]
pub(crate) mod flatten;

pub use block_entity::{BlockEntity, GenericBlockEntity};
pub use block_state::{BlockState, GenericBlockState};
#[allow(unused_imports)]
pub use combined::*;
pub use entity::{Entity, GenericEntity};
