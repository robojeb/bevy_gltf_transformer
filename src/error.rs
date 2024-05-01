//! glTF asset loading error type
use bevy::{
    asset::{AssetLoadError, ReadAssetBytesError},
    render::texture::TextureError,
};
use gltf::{
    accessor::{DataType, Dimensions},
    mesh::Mode,
};
use thiserror::Error;

/// Result type for [GltfTransformer](crate::GltfTransformer) actions
pub type Result<T> = std::result::Result<T, self::Error>;

/// Error type for [GltfTransformer](crate::GltfTransformer) actions
#[derive(Debug, Error)]
pub enum Error {
    /// Unsupported primitive mode.
    #[error("unsupported primitive mode")]
    UnsupportedPrimitive {
        /// The primitive mode.
        mode: Mode,
    },
    /// Invalid glTF file.
    #[error("invalid glTF file: {0}")]
    Gltf(#[from] gltf::Error),
    /// Binary blob is missing.
    #[error("binary blob is missing")]
    MissingBlob,
    /// Decoding the base64 mesh data failed.
    #[error("failed to decode base64 mesh data")]
    Base64Decode(#[from] base64::DecodeError),
    /// Unsupported buffer format.
    #[error("unsupported buffer format")]
    BufferFormatUnsupported,
    /// Invalid image mime type.
    #[error("invalid image mime type: {0}")]
    InvalidImageMimeType(String),
    /// Error when loading a texture. Might be due to a disabled image file format feature.
    #[error("You may need to add the feature for the file format: {0}")]
    ImageError(#[from] TextureError),
    /// Failed to read bytes from an asset path.
    #[error("failed to read bytes from an asset path: {0}")]
    ReadAssetBytesError(#[from] ReadAssetBytesError),
    /// Failed to load asset from an asset path.
    #[error("failed to load asset from an asset path: {0}")]
    AssetLoadError(#[from] AssetLoadError),
    /// Missing sampler for an animation.
    #[error("Missing sampler for animation {0}")]
    MissingAnimationSampler(usize),
    /// Failed to generate tangents.
    #[error("failed to generate tangents: {0}")]
    GenerateTangentsError(#[from] bevy::render::mesh::GenerateTangentsError),
    /// Failed to generate morph targets.
    #[error("failed to generate morph targets: {0}")]
    MorphTarget(#[from] bevy::render::mesh::morph::MorphBuildError),
    /// Direct loading failed
    #[error("failed to load: {0}")]
    LoadDirect(#[from] bevy::asset::LoadDirectError),
    /// Failed to load a file.
    #[error("failed to load file: {0}")]
    Io(#[from] std::io::Error),
    /// Requested an invalid type for an accessor
    #[error("inavlid accessor type: found {requested:?}, expected:  ({dt:?}, {dim:?})")]
    AccessorType {
        /// The type of data requested
        requested: &'static str,
        /// The actual accessor data type
        dt: DataType,
        /// The actual accessor dimensions
        dim: Dimensions,
    },
    /// Could not determine the vertex count of a primitive because it didn't
    /// have Position, Normal, or Tangent information
    #[error("could not determine primitive vertex count")]
    PrimitiveVertexCount,
}
