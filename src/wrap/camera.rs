//! Structures for camera configuration

use super::Document;

/// Information about a Camera's projection
pub struct Camera<'a> {
    _doc: Document<'a>,
    raw: gltf::Camera<'a>,
}

impl<'a> Camera<'a> {
    pub(crate) fn new(doc: Document<'a>, raw: gltf::Camera<'a>) -> Self {
        Self { _doc: doc, raw }
    }

    ///  Returns the internal glTF index
    #[inline(always)]
    pub fn index(&self) -> usize {
        self.raw.index()
    }

    /// Returns the optional user-defined name
    #[inline(always)]
    pub fn name(&self) -> Option<&'a str> {
        self.raw.name()
    }

    /// Returns the camera projection as a bevy projection component
    pub fn projection(&self) -> Projection {
        match self.raw.projection() {
            gltf::camera::Projection::Orthographic(ortho) => {
                Projection::Orthographic(bevy::prelude::OrthographicProjection {
                    near: ortho.znear(),
                    far: ortho.zfar(),
                    ..bevy::prelude::OrthographicProjection::default_3d()
                })
            }
            gltf::camera::Projection::Perspective(persp) => {
                Projection::Perspective(bevy::prelude::PerspectiveProjection {
                    fov: persp.yfov(),
                    aspect_ratio: persp.aspect_ratio().unwrap_or(1.0),
                    near: persp.znear(),
                    far: persp.zfar().unwrap_or(1000.0),
                })
            }
        }
    }

    /// Returns the camera projection as a bevy projection component, defaults
    /// orthographic cameras to the Bevy 2D default.
    pub fn projection_2d(&self) -> Projection {
        match self.raw.projection() {
            gltf::camera::Projection::Orthographic(ortho) => {
                Projection::Orthographic(bevy::prelude::OrthographicProjection {
                    near: ortho.znear(),
                    far: ortho.zfar(),
                    ..bevy::prelude::OrthographicProjection::default_2d()
                })
            }
            gltf::camera::Projection::Perspective(persp) => {
                Projection::Perspective(bevy::prelude::PerspectiveProjection {
                    fov: persp.yfov(),
                    aspect_ratio: persp.aspect_ratio().unwrap_or(1.0),
                    near: persp.znear(),
                    far: persp.zfar().unwrap_or(1000.0),
                })
            }
        }
    }
}

/// A Camera projection
///
/// The glTF file does not specify if the camera is intended to be 2d or 3d but
/// the type of projection typically implies its expected use.
pub enum Projection {
    /// Orthographic "2D" projection
    Orthographic(bevy::prelude::OrthographicProjection),
    /// Perspective "3D" projection
    Perspective(bevy::prelude::PerspectiveProjection),
}
