//! Mask presentation copies only. All detection offsets refer to the original UTF-8 source.
mod detection;
mod pii;
mod request;
mod response;
mod rules;
mod source;
mod syntax;
mod text;
mod transform;

pub(crate) use detection::SourceScan;
pub(crate) use pii::is_supported as is_supported_pii_entity;
pub(crate) use request::{begin_request, is_enabled};
pub(crate) use response::response;
pub(crate) use source::{in_file, named_value, node_text, source};
pub(crate) use transform::hidden;

#[cfg(test)]
mod tests;
