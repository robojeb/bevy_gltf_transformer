//! A libray to generate glTF loader plugins that perform some form of asset
//! transformation at asset load time.
//!
//! This crate exposes two major ways to implement a glTF transformation.
//! The raw [GltfTransformer] trait and the [SimpleGltfTransformer](simple::SimpleGltfTransformer)
//! trait.
//!
//! A raw [GltfTransformer] provides access to a glTF [Document] which is a
//! wrapper around the [gltf] crate's `Document` type. This wrapper provides
//! accessors to automatically translate glTF items into [bevy] types, matching
//! the behavior of the `bevy_gltf` crate when possible.
//!
//! The [GltfTransformer] is responsible for producing an asset from the
//! contents of the [Document], but does not have to conform to any of the glTF
//! specifications or expectations.
//!
//! The [SimpleGltfTransformer](simple::SimpleGltfTransformer) interface provides
//! a quick way to customize meshes and materials while matching the standard
//! `bevy_gltf` loading behavior for other data. This produces a custom
//! equivalent to the [bevy] standard `Gltf` asset.
//!
//! The [Document] also holds a cache of all of the loaded glTF binary data.
//! External binary data is loaded lazily to allow allow transformers which
//! only access part of the glTF data. For example a single glTF asset could
//! hold mesh assets for both "American" and "European" themed buildings.
//! [AssetLoader::Settings] can determine which theme to load, only loading the
//! binary file for the theme that was requested while sharing common meta-data
//! (potentially with custom extensions or extra data).
//!
//! # Missing Features
//!
//! The following features are not currently handled by this crate:
//!
//!  - [ ] glTF animations
//!  - [ ] Mesh morph targets
//!  - [ ] Returning the [gltf] source when using a [SimpleGltfTransformer](simple::SimpleGltfTransformer)
//!  - [ ] Ensure that all glTF wrapper types can return raw data as well as bevy data
//!
//! # FAQ
//!
//! ## Why not use an [AssetProcessor](bevy::asset::processor::AssetProcessor)
//!
//! The [Process](bevy::asset::processor::Process) trait can only manipulate
//! the data present in the loaded asset. For the Bevy Gltf type this consists
//! of handles to other labeled assets.
//!
//! This means that a user wanting to load glTF data as 2D meshes instead of 3D
//! must do it at run-time.
//!
//! ## Why lazy load the buffers?
//!
//! Rather than loading all possible buffer data up-front this library keeps a
//! cache of lazily loaded data. The idea is that some [GltfTransformer]
//! implementations may only want to load a small subset of the underlying
//! glTF data. This allows loaders to treat glTF files as data packages which
//! can reduce the number of files that have to be shipped with a game.
//!
//! # Features I wish [bevy] had
//!
//! There are some features I wish the [bevy] asset system had that would make
//! things more ergonomic or flexible.
//!
//! ## Sparse Asset Loads
//!
//! From a game delivery point of view it would be convenient to be able to pack
//! multiple game levels, or meshes into a single glTF file. Unfortunately at
//! this time [bevy] must load all of labeled assets from a file even if only
//! one item was requested.
//!
//! It would be convenient if an [AssetLoader] could specify that it was
//! capable of sparse-loading, and only provide the requested labeled asset and
//! its dependencies.
//!
//! This would allow glTF files to become the default way to package assets.
//!
//! ## Cached loaded bytes
//!
//! Sparse asset loading becomes a burden if every time an asset is loaded all
//! the backing binary buffers have to be pulled from disk. It would be nice if
//! [bevy] provided a way to `load_asset_bytes_cached()` where the bytes would
//! remain in a memory cache and be reusable between multiple sparse loads.
//! The size of the cache would be determined when configuring the
//! [AssetServer](bevy::asset:AssetServer) allowing bevy to evict not recently
//! used data.
//!
//! ## Labeled loading of external sub-assets or unlabeled sub-assets
//!
//! One weird point in glTF loading is that textures which use a buffer-view
//! data source *must* become a labeled sub-asset, while textures that reference
//! external files can either be labeled by using `load_direct()` or be anonymous
//! with `load()`.
//!
//! This means depending on what the implementor has decided, only some of the
//! textures in a glTF file may be accessible as a labeled asset. It would be
//! nice if there were a `load_as_labeled()` or `add_unlabeled_asset()` to unify
//! handling of these types of assets.
//!

#![warn(missing_docs)]
#![allow(clippy::result_large_err)]
pub mod data;
pub mod error;
pub mod simple;
mod util;
pub mod wrap;

use bevy::asset::{AssetLoader, AsyncReadExt, LoadContext};
use std::{borrow::Cow, future::Future};
use util::{Cache, OwningSlice};
use wrap::{BufferId, Document};

/// A type which reads a glTF file and produces a custom [Asset](bevy::asset::Asset)
pub trait GltfTransformer: Send + Sync + 'static {
    /// The top level [`Asset`] loaded by this [`GltfTransformer`].
    type Asset: bevy::asset::Asset;
    /// The settings type used by this [`GltfTransformer`].
    type Settings: bevy::asset::meta::Settings
        + Default
        + serde::Serialize
        + for<'a> serde::Deserialize<'a>;
    /// The type of [error](`std::error::Error`) which could be encountered by this transformer.
    type Error: Into<Box<dyn std::error::Error + Send + Sync + 'static>>;

    /// Asynchronously loads AssetLoader::Asset (and any other labeled assets) from glTF [Document].
    fn load<'a>(
        &'a self,
        document: Document<'_>,
        settings: &'a Self::Settings,
        ctx: &'a mut LoadContext<'_>,
    ) -> impl Future<Output = Result<Self::Asset, Self::Error>> + Send;

    /// Returns a list of extensions supported by this AssetLoader, without the preceding dot.
    /// Note that users of this AssetLoader may choose to load files with a non-matching extension.
    ///
    /// Defaults to no associated extensions, and requires users to explicitly
    /// utilize this loader when accessing `.gltf` or `.glb` asssets.
    ///
    /// Recommended to use a two level extension if provided like `.glb.2d` for
    /// 2D graphics assets.
    fn extensions(&self) -> &[&str] {
        &[]
    }
}

/// An [AssetLoader] which loads glTF files with a
/// custom [GltfTransformer].
pub struct GltfTransformLoader<T>(pub T);

impl<T: GltfTransformer> AssetLoader for GltfTransformLoader<T> {
    type Asset = T::Asset;
    type Error = T::Error;
    type Settings = T::Settings;

    fn load<'a>(
        &'a self,
        reader: &'a mut bevy::asset::io::Reader,
        settings: &'a Self::Settings,
        load_context: &'a mut bevy::asset::LoadContext<'_>,
    ) -> bevy::utils::BoxedFuture<'a, Result<Self::Asset, Self::Error>> {
        Box::pin(load_gltf(&self.0, reader, settings, load_context))
    }

    fn extensions(&self) -> &[&str] {
        <T as GltfTransformer>::extensions(&self.0)
    }
}

async fn load_gltf<'a, T: GltfTransformer>(
    t: &'a T,
    reader: &'a mut bevy::asset::io::Reader<'_>,
    settings: &'a T::Settings,
    load_context: &'a mut bevy::asset::LoadContext<'_>,
) -> Result<T::Asset, T::Error> {
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer).await.unwrap();
    let buffer = buffer.into_boxed_slice();

    let (document, blob) = match gltf::Glb::from_slice(&buffer) {
        Ok(glb) => {
            let document =
                gltf::Document::from_json(gltf::json::Root::from_slice(&glb.json).unwrap())
                    .unwrap();
            (document, glb.bin)
        }
        Err(_) => {
            let gltf = gltf::Gltf::from_slice(&buffer).unwrap();
            (
                gltf.document,
                None, /* Always none when loading text GLTF */
            )
        }
    };

    // Buffer cache takes ownership of the whole document
    let cache = if let Some(blob) = blob {
        match blob {
            Cow::Owned(o) => Cache::new(OwningSlice::new_complete(o.into_boxed_slice())),
            Cow::Borrowed(s) => {
                let offset = OwningSlice::find_offset(&buffer, s)
                    .expect("Borrowed glTF data chunk was not part of the original buffer, or exceeded the buffer length");
                let slice_len = s.len();
                Cache::new(unsafe { OwningSlice::new(buffer, offset, slice_len) })
            }
        }
    } else {
        util::Cache::empty()
    };

    let doc = wrap::Document {
        doc: &document,
        cache: &cache,
    };

    T::load(t, doc, settings, load_context).await
}
