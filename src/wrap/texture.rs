//! Structures for glTF images and textures
//!
use std::{borrow::Cow, path::PathBuf};

use super::{Document, View};
use crate::{error::Result, util::data_uri::DataUri};
use bevy::{
    asset::{AssetPath, LoadContext},
    image::{
        CompressedImageFormats, Image as BevyImage, ImageAddressMode, ImageFilterMode,
        ImageSampler, ImageSamplerDescriptor, ImageType,
    },
    render::{render_asset::RenderAssetUsages, render_resource::TextureFormat},
};
use gltf::texture::{MagFilter, MinFilter};
use serde_json::{value::RawValue, Value};

macro_rules! magic_check {
    (($mime_type:ident, $buffer:ident) =>$($feature:literal, $magic:expr, $fmt:expr, $err:literal;)*) => {
        if let Some($mime_type) = $mime_type {
            ImageType::MimeType($mime_type)
        } $(
            else if $buffer.starts_with($magic) {
                #[cfg(feature = $feature)]
                {
                    ImageType::Format($fmt)
                }
                #[cfg(not(feature = $feature))]
                {
                    panic!($err)
                }
            }
        )*
        else {
            panic!("Could not identify image type.")
        }
    };
}

/// A raw glTF image. This contains pixel data but no information on texture
/// sampler settings
pub struct Image<'a> {
    doc: Document<'a>,
    raw: gltf::Image<'a>,
}

impl<'a> Image<'a> {
    pub(crate) fn new(doc: Document<'a>, raw: gltf::Image<'a>) -> Self {
        Self { doc, raw }
    }

    /// Returns the internal glTF index
    #[inline(always)]
    pub fn index(&self) -> usize {
        self.raw.index()
    }

    /// Returns the optional user-defined name
    #[inline(always)]
    pub fn name(&self) -> Option<&'a str> {
        self.raw.name()
    }

    /// Returns the data source for this [Image]
    pub fn source(&self) -> Source<'a> {
        match self.raw.source() {
            gltf::image::Source::View { view, mime_type } => Source::View {
                view: View::new(self.doc, view),
                mime_type,
            },
            gltf::image::Source::Uri { uri, mime_type } => {
                let dec_uri = percent_encoding::percent_decode_str(uri)
                    .decode_utf8()
                    .expect(super::URI_ERROR);
                let dec_uri = dec_uri.as_ref();
                match DataUri::parse(dec_uri) {
                    Ok(_) => Source::UriEncoded { uri, mime_type },
                    Err(_) => Source::ExternalPath { uri, mime_type },
                }
            }
        }
    }

    /// Loads the encoded image data directly
    ///
    /// The underlying [Buffer](super::Buffer) or [File] will be added as a
    /// load dependencie to the provided [LoadContext]
    #[inline(always)]
    pub async fn load_direct(&self, ctx: &mut LoadContext<'_>) -> Result<Cow<'a, [u8]>> {
        self.source().load_direct(ctx).await
    }

    /// Loads the image as a bevy texture ([Image](BevyImage))
    /// with the specified settings.
    pub async fn load(
        &self,
        ctx: &mut LoadContext<'_>,
        settings: ImageLoadSettings,
    ) -> Result<BevyImage> {
        let source: Source<'a> = self.source();

        let loaded: BevyImage = match source {
            Source::View { view, mime_type } => {
                let data = view.load(ctx).await?;

                BevyImage::from_buffer(
                    #[cfg(all(debug_assertions, feature = "dds"))]
                    format!("Image({}, {:?})", self.index, settings),
                    data,
                    ImageType::MimeType(mime_type),
                    CompressedImageFormats::all(),
                    settings.is_srgb,
                    settings.sampler,
                    settings.asset_usage,
                )?
            }
            Source::UriEncoded { uri, mime_type } => {
                // NOTE: Magic numbers are not guarded under features so that
                // the proper error messages can be reported to the user.
                const PNG_MAGIC: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
                const JPEG_MAGIC: &[u8] = &[0xFF, 0xD8, 0xFF];
                const QOI_MAGIC: &[u8] = b"qoif";
                const EXR_MAGIC: &[u8] = &[0x76, 0x2F, 0x31, 0x01];
                const GIF_MAGIC_A: &[u8] = b"GIF87a";
                const GIF_MAGIC_B: &[u8] = b"GIF89a";

                let uri = percent_encoding::percent_decode_str(uri)
                    .decode_utf8()
                    .expect(super::URI_ERROR);
                let uri = uri.as_ref();

                let buffer_bytes = match DataUri::parse(uri) {
                    Ok(data_uri) => data_uri.decode()?,
                    _ => unreachable!(),
                };

                // Try to get the MIME Type
                let image_type = magic_check!((mime_type, buffer_bytes) =>
                    "png", PNG_MAGIC, bevy::image::ImageFormat::Png, "PNG loading requires the `png` feature.";
                    "jpeg", JPEG_MAGIC, bevy::image::ImageFormat::Jpeg, "JPEG loading requires the `jpeg` feature.";
                    "qoi", QOI_MAGIC, bevy::image::ImageFormat::Qoi, "QOI loading requires the `qoi` feature.";
                    "exr", EXR_MAGIC, bevy::image::ImageFormat::OpenExr, "OpenEXR loading requires the `exr` feature.";
                    "gif", GIF_MAGIC_A, bevy::image::ImageFormat::Gif, "Gif loading requires the `gif` feature.";
                    "gif", GIF_MAGIC_B, bevy::image::ImageFormat::Gif, "Gif loading requires the `gif` feature.";
                    "ff", b"farbfeld", bevy::image::ImageFormat::Farbfeld, "Farbfeld loading requires the `ff` feature.";
                    // BMP file magic numbers
                    "bmp", b"BM", bevy::image::ImageFormat::Bmp, "Bmp loading requires the `bmp` feature";
                    "bmp", b"BA", bevy::image::ImageFormat::Bmp, "Bmp loading requires the `bmp` feature";
                    "bmp", b"CI", bevy::image::ImageFormat::Bmp, "Bmp loading requires the `bmp` feature";
                    "bmp", b"CP", bevy::image::ImageFormat::Bmp, "Bmp loading requires the `bmp` feature";
                    "bmp", b"IC", bevy::image::ImageFormat::Bmp, "Bmp loading requires the `bmp` feature";
                    "bmp", b"PT", bevy::image::ImageFormat::Bmp, "Bmp loading requires the `bmp` feature";
                    // Several Netbpm types
                    "pnm", b"P1", bevy::image::ImageFormat::Pnm, "PBM loading requires the `pnm` feature.";
                    "pnm", b"P4", bevy::image::ImageFormat::Pnm, "PBM loading requires the `pnm` feature.";
                    "pnm", b"P2", bevy::image::ImageFormat::Pnm, "PGM loading requires the `pnm` feature.";
                    "pnm", b"P5", bevy::image::ImageFormat::Pnm, "PGM loading requires the `pnm` feature.";
                    "pnm", b"P3", bevy::image::ImageFormat::Pnm, "PPM loading requires the `pnm` feature.";
                    "pnm", b"P6", bevy::image::ImageFormat::Pnm, "PPM loading requires the `pnm` feature.";
                    // TODO:   Basis, HDR, ICO, KTX2, TGA, TIFF, Webp
                );

                BevyImage::from_buffer(
                    #[cfg(all(debug_assertions, feature = "dds"))]
                    format!("Image({}, {:?})", self.index, settings),
                    &buffer_bytes,
                    image_type,
                    CompressedImageFormats::all(),
                    settings.is_srgb,
                    settings.sampler,
                    settings.asset_usage,
                )?
            }
            Source::ExternalPath { uri, .. } => {
                let path = PathBuf::from(uri);
                let asset_path = AssetPath::from(path);

                let mut loaded = ctx
                    .loader()
                    .immediate()
                    .load::<BevyImage>(asset_path)
                    .await?
                    .take();

                // Apply our settings
                loaded.asset_usage = settings.asset_usage;
                loaded.sampler = settings.sampler;
                loaded.texture_descriptor.format =
                    transform_format(loaded.texture_descriptor.format, settings.is_srgb);

                loaded
            }
        };

        Ok(loaded)
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

fn transform_format(fmt: TextureFormat, is_srgb: bool) -> TextureFormat {
    match fmt {
        TextureFormat::Rgba8Unorm if is_srgb => TextureFormat::Rgba8UnormSrgb,
        TextureFormat::Rgba8UnormSrgb if !is_srgb => TextureFormat::Rgba8Unorm,
        TextureFormat::Bgra8Unorm if is_srgb => TextureFormat::Bgra8UnormSrgb,
        TextureFormat::Bgra8UnormSrgb if !is_srgb => TextureFormat::Bgra8Unorm,
        TextureFormat::Bc1RgbaUnorm if is_srgb => TextureFormat::Bc1RgbaUnormSrgb,
        TextureFormat::Bc1RgbaUnormSrgb if !is_srgb => TextureFormat::Bc1RgbaUnorm,
        TextureFormat::Bc2RgbaUnorm if is_srgb => TextureFormat::Bc2RgbaUnormSrgb,
        TextureFormat::Bc2RgbaUnormSrgb if !is_srgb => TextureFormat::Bc2RgbaUnorm,
        TextureFormat::Bc3RgbaUnorm if is_srgb => TextureFormat::Bc3RgbaUnormSrgb,
        TextureFormat::Bc3RgbaUnormSrgb if !is_srgb => TextureFormat::Bc3RgbaUnorm,
        TextureFormat::Bc7RgbaUnorm if is_srgb => TextureFormat::Bc7RgbaUnormSrgb,
        TextureFormat::Bc7RgbaUnormSrgb if !is_srgb => TextureFormat::Bc7RgbaUnorm,
        TextureFormat::Etc2Rgb8Unorm if is_srgb => TextureFormat::Etc2Rgb8UnormSrgb,
        TextureFormat::Etc2Rgb8UnormSrgb if !is_srgb => TextureFormat::Etc2Rgb8Unorm,
        TextureFormat::Etc2Rgb8A1Unorm if is_srgb => TextureFormat::Etc2Rgb8A1UnormSrgb,
        TextureFormat::Etc2Rgb8A1UnormSrgb if !is_srgb => TextureFormat::Etc2Rgb8A1Unorm,
        TextureFormat::Etc2Rgba8Unorm if is_srgb => TextureFormat::Etc2Rgba8UnormSrgb,
        TextureFormat::Etc2Rgba8UnormSrgb if !is_srgb => TextureFormat::Etc2Rgba8Unorm,
        x => x,
    }
}

/// Minimal settings for loading an [Image] as a [Bevy texture](bevy::render::texture::Image)
#[derive(Debug)]
pub struct ImageLoadSettings {
    /// Should the data be treated as sRGB
    pub is_srgb: bool,
    /// Definition for texture sampling
    pub sampler: ImageSampler,
    /// Expected usage of the image data
    pub asset_usage: RenderAssetUsages,
}

/// The source for [Image] data
pub enum Source<'a> {
    /// The image data resids in a buffer [View]
    View {
        /// The [View] containing the data
        view: View<'a>,
        /// The mime-type for loading the view
        mime_type: &'a str,
    },
    /// The image data is encoded directly in the URI
    UriEncoded {
        /// The data URI
        uri: &'a str,
        /// The optional mime-type for loading the data
        ///
        /// If the mime-type is not specified the loader should probe the first
        /// few decoded bytes to determine the image format.
        mime_type: Option<&'a str>,
    },
    /// The image data is stored in an external path
    ExternalPath {
        /// The path for loading the image data
        uri: &'a str,
        /// The optional mime-type for loading the data
        ///
        /// If the mime-type is not specified the loader should examine the
        /// extension and/or the first few decoded bytes to determine the
        /// image format.
        mime_type: Option<&'a str>,
    },
}

impl<'a> Source<'a> {
    /// Loads the encoded image data directly
    ///
    /// The underlying [Buffer](super::Buffer) or [File](std::fs::File) will be
    /// added as a load dependencie to the provided [LoadContext]
    pub async fn load_direct(&self, ctx: &mut LoadContext<'_>) -> Result<Cow<'a, [u8]>> {
        match self {
            Self::View { view, .. } => view.load(ctx).await.map(Cow::Borrowed),
            Self::UriEncoded { uri, .. } | Self::ExternalPath { uri, .. } => {
                let uri = percent_encoding::percent_decode_str(uri)
                    .decode_utf8()
                    .expect(super::URI_ERROR);
                let uri = uri.as_ref();

                let data = if let Ok(data_uri) = DataUri::parse(uri) {
                    data_uri.decode()?
                } else {
                    let image_path = ctx.path().parent().unwrap().join(uri);
                    ctx.read_asset_bytes(image_path).await?
                };

                Ok(Cow::Owned(data))
            }
        }
    }

    /// Gets the MIME type of the image if provided
    pub fn mime_type(&self) -> Option<&'a str> {
        match self {
            Self::View { mime_type, .. } => Some(*mime_type),
            Self::UriEncoded { mime_type, .. } | Self::ExternalPath { mime_type, .. } => *mime_type,
        }
    }
}

/// A glTF texture consisting of an [Image] and [Sampler] information
pub struct Texture<'a> {
    doc: Document<'a>,
    raw: gltf::Texture<'a>,
}

impl<'a> Texture<'a> {
    pub(crate) fn new(doc: Document<'a>, raw: gltf::Texture<'a>) -> Self {
        Self { doc, raw }
    }

    /// The underlying [Image] that provides the texel data
    pub fn source(&self) -> Image<'a> {
        Image::new(self.doc, self.raw.source())
    }

    /// Definitions for the texture sampler
    pub fn sampler(&self) -> Sampler<'a> {
        Sampler::new(self.doc, self.raw.sampler())
    }

    /// Load the [Texture] into the appropriate bevy type
    pub async fn load(
        &self,
        ctx: &mut LoadContext<'_>,
        is_srgb: bool,
        asset_usage: RenderAssetUsages,
    ) -> Result<BevyImage> {
        self.source()
            .load(
                ctx,
                ImageLoadSettings {
                    is_srgb,
                    sampler: self.sampler().as_bevy_sampler(),
                    asset_usage,
                },
            )
            .await
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

/// glTF texture sampling information
pub struct Sampler<'a> {
    _doc: Document<'a>,
    raw: gltf::texture::Sampler<'a>,
}

impl<'a> Sampler<'a> {
    pub(crate) fn new(doc: Document<'a>, raw: gltf::texture::Sampler<'a>) -> Self {
        Self { _doc: doc, raw }
    }

    /// Converts the glTF information to a bevy sampler
    ///
    /// This follows the method used by the default bevy Gltf loader.
    pub fn as_bevy_sampler(&self) -> ImageSampler {
        ImageSampler::Descriptor(ImageSamplerDescriptor {
            label: self.raw.name().map(String::from),
            address_mode_u: match self.raw.wrap_s() {
                gltf::texture::WrappingMode::ClampToEdge => ImageAddressMode::ClampToEdge,
                gltf::texture::WrappingMode::MirroredRepeat => ImageAddressMode::MirrorRepeat,
                gltf::texture::WrappingMode::Repeat => ImageAddressMode::Repeat,
            },
            address_mode_v: match self.raw.wrap_t() {
                gltf::texture::WrappingMode::ClampToEdge => ImageAddressMode::ClampToEdge,
                gltf::texture::WrappingMode::MirroredRepeat => ImageAddressMode::MirrorRepeat,
                gltf::texture::WrappingMode::Repeat => ImageAddressMode::Repeat,
            },
            address_mode_w: ImageAddressMode::ClampToEdge,
            mag_filter: match self.raw.mag_filter() {
                Some(MagFilter::Nearest) => ImageFilterMode::Nearest,
                _ => ImageFilterMode::Linear,
            },
            min_filter: match self.raw.min_filter() {
                Some(MinFilter::Nearest) => ImageFilterMode::Nearest,
                _ => ImageFilterMode::Linear,
            },
            ..Default::default()
        })
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
