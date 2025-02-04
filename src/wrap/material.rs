//! Structures for glTF material definitions
use bevy::color::Color;
use serde_json::{value::RawValue, Value};

use super::Document;

/// Information about a glTF material
///
/// The glTF model assumes a PBR style pipeline using a metallic-roughness
/// model by default. An optional extension exists which supports a
/// specular-glossiness PBR model.
pub struct Material<'a> {
    _doc: Document<'a>,
    raw: gltf::Material<'a>,
}

impl<'a> Material<'a> {
    pub(crate) fn new(doc: Document<'a>, raw: gltf::Material<'a>) -> Self {
        Self { _doc: doc, raw }
    }

    /// The internal glTF index for this [Material]
    pub fn index(&self) -> Option<usize> {
        self.raw.index()
    }

    /// Returns the optional user-provided name
    pub fn name(&self) -> Option<&'a str> {
        self.raw.name()
    }

    /// The optional alpha cutoff value of the material.
    #[inline(always)]
    pub fn alpha_cutoff(&self) -> Option<f32> {
        self.raw.alpha_cutoff()
    }

    /// The alpha rendering mode of the material.  The material's alpha rendering
    /// mode enumeration specifying the interpretation of the alpha value of the main
    /// factor and texture.
    ///
    /// * In `Opaque` mode (default) the alpha value is ignored
    ///   and the rendered output is fully opaque.
    /// * In `Mask` mode, the rendered
    ///   output is either fully opaque or fully transparent depending on the alpha
    ///   value and the specified alpha cutoff value.
    /// * In `Blend` mode, the alpha value is used to composite the source and
    ///   destination areas and the rendered output is combined with the background
    ///   using the normal painting operation (i.e. the Porter and Duff over
    ///   operator).
    #[inline(always)]
    pub fn alpha_mode(&self) -> gltf::material::AlphaMode {
        self.raw.alpha_mode()
    }

    /// Specifies whether the material is double-sided.
    ///
    /// * When this value is false, back-face culling is enabled.
    /// * When this value is true, back-face culling is disabled and double sided
    ///   lighting is enabled.  The back-face must have its normals reversed before
    ///   the lighting equation is evaluated.
    #[inline(always)]
    pub fn double_sided(&self) -> bool {
        self.raw.double_sided()
    }

    /// Parameter values that define the metallic-roughness material model from Physically-Based Rendering (PBR) methodology.
    #[inline(always)]
    pub fn pbr_base(&self) -> PBRMetallicRoughness<'a> {
        PBRMetallicRoughness::new(self._doc, self.raw.pbr_metallic_roughness())
    }

    /// Get the base color for this [Material] from its [PBRMetallicRoughness] parameters
    ///
    /// This assumes the color is stored in the sRGB color space.
    /// If you need raw access to the underlying values use [Self::pbr_base()]
    /// and [PBRBase::base_color_value()].
    ///
    /// The default value is [Color::WHITE].
    pub fn base_color(&self) -> Color {
        let pbr = self.raw.pbr_metallic_roughness();
        Color::srgba(
            pbr.base_color_factor()[0],
            pbr.base_color_factor()[1],
            pbr.base_color_factor()[2],
            pbr.base_color_factor()[3],
        )
    }

    /// Returns the metalness factor of the material.
    ///
    /// The default value is 1.0.
    pub fn metallic(&self) -> f32 {
        let pbr = self.raw.pbr_metallic_roughness();
        pbr.metallic_factor()
    }

    /// Returns the roughness factor of the material.
    ///
    /// * A value of 1.0 means the material is completely rough.
    /// * A value of 0.0 means the material is completely smooth.
    ///
    /// The default value is 1.0.
    pub fn perceptual_roughness(&self) -> f32 {
        let pbr = self.raw.pbr_metallic_roughness();
        pbr.roughness_factor()
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

/// Material information using the PBR Metallic-Roughness model
pub struct PBRMetallicRoughness<'a> {
    _doc: Document<'a>,
    raw: gltf::material::PbrMetallicRoughness<'a>,
}

impl<'a> PBRMetallicRoughness<'a> {
    pub(crate) fn new(doc: Document<'a>, raw: gltf::material::PbrMetallicRoughness<'a>) -> Self {
        Self { _doc: doc, raw }
    }

    /// Returns the material’s base color factor.
    ///
    /// The default value is `[1.0, 1.0, 1.0, 1.0]`.
    pub fn base_color_value(&self) -> [f32; 4] {
        self.raw.base_color_factor()
    }
}
