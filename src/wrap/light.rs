//! Structures for glTF lights from the `KHR_lights_punctual` extension
//!
use super::Document;
use bevy::render::color::Color;
use gltf::khr_lights_punctual::Kind;
use serde_json::value::RawValue;

/// A glTF light from the `KHR_lights_punctual` extension
pub struct Light<'a> {
    _doc: Document<'a>,
    raw: gltf::khr_lights_punctual::Light<'a>,
}

impl<'a> Light<'a> {
    pub(crate) fn new(doc: Document<'a>, raw: gltf::khr_lights_punctual::Light<'a>) -> Self {
        Self { _doc: doc, raw }
    }

    /// Color of the light source
    ///
    /// This assumes the color is in the sRGB color-space.
    pub fn color(&self) -> Color {
        Color::rgb_from_array(self.raw.color())
    }

    /// The internal glTF index
    #[inline(always)]
    pub fn index(&self) -> usize {
        self.raw.index()
    }

    /// Optional user-defined name for this light
    #[inline(always)]
    pub fn name(&self) -> Option<&'a str> {
        self.raw.name()
    }

    /// Intensity of the light source defined in glTF
    ///
    /// For [Kind::Point] and [Kind::Spot] lights this is luminosity
    /// in [candela](https://en.wikipedia.org/wiki/Candela) (lm/sr) while
    /// [Kind::Directional] lights use [lux](https://en.wikipedia.org/wiki/Lux)
    /// (lm/m^2).
    #[inline(always)]
    pub fn intensity(&self) -> f32 {
        self.raw.intensity()
    }

    /// Intensity of the light source in units appropriate to Bevy lights.
    ///
    /// For [Kind::Point] and [Kind::Spot] this will convert from [candela](https://en.wikipedia.org/wiki/Candela)
    /// (lm/sr) to [lumens](https://en.wikipedia.org/wiki/Lumen_(unit)).
    /// For [Kind::Directional] this performs no conversion as Bevy already expects
    /// [lux](https://en.wikipedia.org/wiki/Lux) (lm/m^2).
    pub fn intensity_bevy(&self) -> f32 {
        match self.kind() {
            Kind::Directional => self.intensity(),
            // NOTE: KHR_punctual_lights defines the intensity units for point lights in
            // candela (lm/sr) which is luminous intensity and we need luminous power.
            // For a point light, luminous power = 4 * pi * luminous intensity
            Kind::Point | Kind::Spot { .. } => self.intensity() * std::f32::consts::PI * 4.0,
        }
    }

    /// Distance cutoff (meters) after which the light's intensity may be
    /// considered to have reached zero
    #[inline(always)]
    pub fn range(&self) -> Option<f32> {
        self.raw.range()
    }

    /// The kind of light
    pub fn kind(&self) -> Kind {
        self.raw.kind()
    }

    /// Application specific extra information as raw JSON data.
    pub fn extras(&self) -> Option<&RawValue> {
        self.raw.extras().as_deref()
    }

    /// Converts this [Light] into its corresponding Bevy light type.
    ///
    /// This uses the same conversion as the default Bevy glTF crate.
    #[cfg(feature = "bevy_3d")]
    pub fn as_bevy_light(&self) -> LightKind {
        use bevy::pbr::{DirectionalLight, PointLight, SpotLight};

        match self.raw.kind() {
            Kind::Directional => LightKind::Directional(DirectionalLight {
                color: self.color(),
                illuminance: self.intensity_bevy(),
                ..Default::default()
            }),
            Kind::Point => LightKind::Point(PointLight {
                color: self.color(),
                intensity: self.intensity_bevy(),
                range: self.range().unwrap_or(2.0),
                radius: 0.0,
                ..Default::default()
            }),

            Kind::Spot {
                inner_cone_angle,
                outer_cone_angle,
            } => LightKind::Spot(SpotLight {
                color: self.color(),
                intensity: self.intensity_bevy(),
                range: self.range().unwrap_or(20.0),
                radius: self.range().unwrap_or(0.0),
                inner_angle: inner_cone_angle,
                outer_angle: outer_cone_angle,
                ..Default::default()
            }),
        }
    }
}

/// One of Bevy's PBR light types
#[cfg(feature = "bevy_3d")]
pub enum LightKind {
    /// A directional "sun" light
    Directional(bevy::pbr::DirectionalLight),
    /// A spot light
    Spot(bevy::pbr::SpotLight),
    /// A point light
    Point(bevy::pbr::PointLight),
}
