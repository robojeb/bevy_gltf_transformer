//! Structures for glTF buffers and buffer-views
//!

use super::{BufferId, Document};
use crate::{
    error::{Error, Result},
    util::data_uri::DataUri,
};
use bevy::asset::LoadContext;
use gltf::buffer::Source;
use serde_json::{value::RawValue, Value};

/// Meta-data for a buffer from a glTF file
pub struct Buffer<'a> {
    doc: Document<'a>,
    raw: gltf::Buffer<'a>,
}

impl<'a> Buffer<'a> {
    pub(crate) fn new(doc: Document<'a>, raw: gltf::Buffer<'a>) -> Self {
        Self { doc, raw }
    }

    /// Load the data from this [Buffer]
    ///
    /// If the buffer data is external to the glTF file it will be added as
    /// a load dependency of the provided [LoadContext]
    // NOTE: False positive due to explicitly dropepd guard. Cannot use lint
    // workaround because of the required control-flow.
    #[allow(clippy::await_holding_lock)]
    pub async fn load(&self, ctx: &mut LoadContext<'_>) -> Result<&'a [u8]> {
        let id = match self.raw.source() {
            Source::Bin => BufferId::Bin,
            Source::Uri(_) => BufferId::Buffer(self.raw.index()),
        };

        match self.doc.inner.cache.get(id) {
            Some(buf) => Ok(buf),
            None => {
                let Source::Uri(uri) = self.raw.source() else {
                    panic!("GLB Binary data should always be cached.")
                };

                let uri = percent_encoding::percent_decode_str(uri)
                    .decode_utf8()
                    .expect(super::URI_ERROR);
                let uri = uri.as_ref();

                let buffer_bytes = match DataUri::parse(uri) {
                    Ok(data_uri) if super::VALID_MIME_TYPES.contains(&data_uri.mime_type) => {
                        data_uri.decode()?
                    }
                    Ok(_) => return Err(Error::BufferFormatUnsupported),
                    Err(_) => {
                        let buffer_path = ctx.path().parent().unwrap().join(uri);
                        ctx.read_asset_bytes(buffer_path).await?
                    }
                };

                let cached = buffer_bytes.into_boxed_slice();
                Ok(self.doc.inner.cache.store(id, cached))
            }
        }
    }

    /// The length of the buffer in bytes.
    #[inline(always)]
    pub fn length(&self) -> usize {
        self.raw.length()
    }

    /// Returns the buffer data source.
    #[inline(always)]
    pub fn source(&self) -> Source<'a> {
        self.raw.source()
    }

    /// Check if this item has data for the named extension
    pub fn has_extension(&self, name: &str) -> bool {
        self.raw.extension_value(name).is_some()
    }

    /// Get the raw JSON data for the named extension if present
    pub fn extension_value(&self, name: &str) -> Option<&Value> {
        self.raw.extension_value(name)
    }

    /// Application specific extra information as raw JSON data.
    pub fn extras(&self) -> Option<&RawValue> {
        self.raw.extras().as_deref()
    }
}

/// A view into a specific [Buffer] from a glTF file
pub struct View<'a> {
    doc: Document<'a>,
    raw: gltf::buffer::View<'a>,
}

impl<'a> View<'a> {
    pub(crate) fn new(doc: Document<'a>, raw: gltf::buffer::View<'a>) -> Self {
        Self { doc, raw }
    }

    /// Returns the parent [Buffer]
    #[inline(always)]
    pub fn buffer(&self) -> Buffer<'a> {
        Buffer::new(self.doc, self.raw.buffer())
    }

    /// Returns the offset into the parent buffer in bytes.
    #[inline(always)]
    pub fn offset(&self) -> usize {
        self.raw.offset()
    }

    /// The length of the buffer view in bytes.
    #[inline(always)]
    pub fn length(&self) -> usize {
        self.raw.length()
    }

    /// Returns the stride in bytes between vertex attributes or other interleavable data. When None, data is assumed to be tightly packed.
    #[inline(always)]
    pub fn stride(&self) -> Option<usize> {
        self.raw.stride()
    }

    /// Load the data from this [View]
    ///
    /// If the parent buffer data is external to the glTF file it will be added
    /// as a load dependency of the provided [LoadContext]
    #[inline(always)]
    pub async fn load(&self, ctx: &mut LoadContext<'_>) -> Result<&'a [u8]> {
        Ok(&self.buffer().load(ctx).await?[self.offset()..])
    }

    /// Check if this item has data for the named extension
    pub fn has_extension(&self, name: &str) -> bool {
        self.raw.extension_value(name).is_some()
    }

    /// Get the raw JSON data for the named extension if present
    pub fn extension_value(&self, name: &str) -> Option<&Value> {
        self.raw.extension_value(name)
    }

    /// Application specific extra information as raw JSON data.
    pub fn extras(&self) -> Option<&RawValue> {
        self.raw.extras().as_deref()
    }
}
