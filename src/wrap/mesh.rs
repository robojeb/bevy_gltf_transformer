//! Structures for glTF Mesh and Primitive data
//!
use super::{iter::Primitives, Accessor, Document, ElementShape, ElementType, Material};
use crate::error::{Error, Result};
use bevy::{
    asset::LoadContext,
    math::{bounding::Aabb3d, f32::Vec3},
    render::{
        mesh::{Indices, Mesh as BevyMesh, PrimitiveTopology, VertexAttributeValues},
        render_asset::RenderAssetUsages,
    },
};
#[cfg(feature = "bevy_3d")]
use bevy::{ecs::world::World, scene::Scene as BevyScene};

use gltf::{mesh::Mode, Semantic};
use serde_json::{value::RawValue, Value};

/// A single primitive for a [Mesh] in a glTF file
#[derive(Clone)]
pub struct Primitive<'a> {
    _doc: Document<'a>,
    raw: gltf::Primitive<'a>,
}

impl<'a> Primitive<'a> {
    pub(crate) fn new(doc: Document<'a>, raw: gltf::Primitive<'a>) -> Self {
        Self { _doc: doc, raw }
    }

    /// Get the internal glTF index of this [Primitive]
    #[inline(always)]
    pub fn index(&self) -> usize {
        self.raw.index()
    }

    /// Get the bounding box of the `POSITION` vertex attribute
    pub fn bounding_box(&self) -> Aabb3d {
        let gltf::mesh::BoundingBox { min, max } = self.raw.bounding_box();
        Aabb3d {
            min: Vec3::from(min),
            max: Vec3::from(max),
        }
    }

    /// Returns the material to apply to this primitive when rendering
    pub fn material(&self) -> Material<'a> {
        Material::new(self._doc, self.raw.material())
    }

    /// Returns the topology of the primitive
    ///
    /// If the glTF specified topology is not supported by Bevy
    /// [Error::PrimitiveTopology] will be returned with the glTF mode.
    pub fn topology(&self) -> Result<PrimitiveTopology> {
        match self.raw.mode() {
            Mode::Points => Ok(PrimitiveTopology::PointList),
            Mode::Lines => Ok(PrimitiveTopology::LineList),
            Mode::LineStrip => Ok(PrimitiveTopology::LineStrip),
            Mode::Triangles => Ok(PrimitiveTopology::TriangleList),
            Mode::TriangleStrip => Ok(PrimitiveTopology::TriangleStrip),
            x => Err(Error::UnsupportedPrimitive { mode: x }),
        }
    }

    /// Get the accessor for the requested vertex attribute
    pub fn get_accessor(&self, semantic: &Semantic) -> Option<Accessor<'a>> {
        self.raw.get(semantic).map(|a| Accessor::new(self._doc, a))
    }

    /// Loads this primitive as a standard 3D Bevy [Mesh](BevyMesh)
    ///
    /// Currently this will only load Posittion, Normal, Tangent, and UV{0,1}
    /// attributes.
    pub async fn as_mesh(
        &self,
        ctx: &mut LoadContext<'_>,
        asset_usage: RenderAssetUsages,
    ) -> Result<BevyMesh> {
        let mut mesh = BevyMesh::new(self.topology()?, asset_usage);

        for (attr, raw_accessor) in self.raw.attributes() {
            let accessor = Accessor::new(self._doc, raw_accessor);

            let (attr, value) = match attr {
                Semantic::Positions => match accessor.shape() {
                    ElementShape::Vec3(ElementType::F32) => (
                        BevyMesh::ATTRIBUTE_POSITION,
                        VertexAttributeValues::Float32x3(
                            accessor.load::<[f32; 3]>(ctx).await?.iter().collect(),
                        ),
                    ),
                    ElementShape::Vec2(ElementType::F32) => (
                        BevyMesh::ATTRIBUTE_POSITION,
                        VertexAttributeValues::Float32x3(
                            accessor
                                .load::<[f32; 2]>(ctx)
                                .await?
                                .iter()
                                .map(|[x, z]| [x, 0.0, z])
                                .collect(),
                        ),
                    ),
                    _ => todo!("New error for invalid conversion"),
                },
                Semantic::Normals => match accessor.shape() {
                    ElementShape::Vec3(ElementType::F32) => (
                        BevyMesh::ATTRIBUTE_NORMAL,
                        VertexAttributeValues::Float32x3(
                            accessor.load::<[f32; 3]>(ctx).await?.iter().collect(),
                        ),
                    ),
                    _ => todo!("New error for invalid conversion"),
                },
                Semantic::Tangents => match accessor.shape() {
                    ElementShape::Vec3(ElementType::F32) => (
                        BevyMesh::ATTRIBUTE_TANGENT,
                        VertexAttributeValues::Float32x3(
                            accessor.load::<[f32; 3]>(ctx).await?.iter().collect(),
                        ),
                    ),
                    _ => todo!("New error for invalid conversion"),
                },
                Semantic::TexCoords(c) if (0..=1).contains(&c) => match accessor.shape() {
                    ElementShape::Vec2(ElementType::F32) => (
                        match c {
                            0 => BevyMesh::ATTRIBUTE_UV_0,
                            1 => BevyMesh::ATTRIBUTE_UV_1,
                            _ => unreachable!(),
                        },
                        VertexAttributeValues::Float32x2(
                            accessor.load::<[f32; 2]>(ctx).await?.iter().collect(),
                        ),
                    ),
                    _ => todo!("New error for invalid conversion"),
                },
                _ => continue,
            };

            mesh.insert_attribute(attr, value);
        }

        if let Some(raw_index_accessor) = self.raw.indices() {
            let indices = Accessor::new(self._doc, raw_index_accessor);

            let indices = match indices.shape() {
                ElementShape::Scalar(ElementType::U8) => Indices::U16(
                    indices
                        .load::<u8>(ctx)
                        .await?
                        .iter()
                        .map(|i| i as u16)
                        .collect(),
                ),
                ElementShape::Scalar(ElementType::U16) => {
                    Indices::U16(indices.load::<u16>(ctx).await?.iter().collect())
                }
                ElementShape::Scalar(ElementType::U32) => {
                    Indices::U32(indices.load::<u32>(ctx).await?.iter().collect())
                }
                _ => todo!("Invalid index type"),
            };

            mesh.insert_indices(indices);
        }

        Ok(mesh)
    }
}

/// A mesh in a glTF file
///
/// This may consist of multiple [Primitives] each with a potentially different
/// [Material].
pub struct Mesh<'a> {
    _doc: Document<'a>,
    raw: gltf::Mesh<'a>,
}

impl<'a> Mesh<'a> {
    pub(crate) fn new(doc: Document<'a>, raw: gltf::Mesh<'a>) -> Self {
        Self { _doc: doc, raw }
    }

    /// Returns the internal glTF index for this [Mesh]
    #[inline(always)]
    pub fn index(&self) -> usize {
        self.raw.index()
    }

    /// Returns the optional user-provided name
    #[inline(always)]
    pub fn name(&self) -> Option<&'a str> {
        self.raw.name()
    }

    /// Returns an [Iterator] over all of the [Primitives] of this [Mesh]
    pub fn primitives(&self) -> Primitives<'a> {
        Primitives::new(self._doc, self.raw.primitives())
    }

    /// Generates a [Scene](BevyScene) that loads all of the [Primitive]s as
    /// as [Entities](bevy::prelude::Entity).
    ///
    /// All materials will be loaded as [StandardMaterial](bevy::pbr::StandardMaterial).
    #[cfg(feature = "bevy_3d")]
    pub async fn as_bevy_scene(
        &self,
        ctx: &mut LoadContext<'_>,
        asset_usage: RenderAssetUsages,
    ) -> Result<BevyScene> {
        use bevy::{
            pbr::{MaterialMeshBundle, StandardMaterial},
            render::color::Color,
        };

        let mut world = World::new();

        for prim in self.primitives() {
            let mesh = prim.as_mesh(ctx, asset_usage).await?;
            let mesh = ctx.add_labeled_asset(
                format!("mesh/{}/primitive/{}", self.raw.index(), prim.index()),
                mesh,
            );

            // FIXME: Should actually load the material
            let material = StandardMaterial::from(Color::WHITE);
            let material = ctx.add_labeled_asset(
                format!("mesh/{}/material/{}", self.raw.index(), 0),
                material,
            );

            world.spawn(MaterialMeshBundle {
                mesh,
                material,
                ..Default::default()
            });
        }

        Ok(BevyScene::new(world))
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

    /// Optional morph target weights
    pub fn weights(&self) -> Option<&'a [f32]> {
        self.raw.weights()
    }
}

/// Accessors for reading mesh vertex attributes
pub mod attributes {
    use crate::{
        data::Accessible,
        wrap::{ElementShape, ElementType},
    };

    /// Reads accessor data into values for `Mesh::ATTRIBUTE_COLOR`
    pub struct AttrColor;

    impl Accessible for AttrColor {
        type Item = [f32; 4];

        fn validate_accessor(shape: crate::wrap::ElementShape) -> bool {
            matches!(
                shape,
                ElementShape::Vec3(ElementType::F32 | ElementType::U16 | ElementType::U8)
                    | ElementShape::Vec4(ElementType::F32 | ElementType::U16 | ElementType::U8)
            )
        }

        fn zero(_shape: ElementShape) -> Self::Item {
            [0.0; 4]
        }

        fn from_element(mut elem: crate::data::Element) -> Self::Item {
            match elem.shape {
                ElementShape::Vec3(ElementType::F32) => {
                    [elem.read_f32(), elem.read_f32(), elem.read_f32(), 1.0]
                }
                ElementShape::Vec4(ElementType::F32) => [
                    elem.read_f32(),
                    elem.read_f32(),
                    elem.read_f32(),
                    elem.read_f32(),
                ],

                ElementShape::Vec3(ElementType::U8) => [
                    elem.read_u8() as f32 / 255.0,
                    elem.read_u8() as f32 / 255.0,
                    elem.read_u8() as f32 / 255.0,
                    1.0,
                ],
                ElementShape::Vec4(ElementType::U8) => [
                    elem.read_u8() as f32 / 255.0,
                    elem.read_u8() as f32 / 255.0,
                    elem.read_u8() as f32 / 255.0,
                    elem.read_u8() as f32 / 255.0,
                ],

                ElementShape::Vec3(ElementType::U16) => [
                    elem.read_u16() as f32 / 65535.0,
                    elem.read_u16() as f32 / 65535.0,
                    elem.read_u16() as f32 / 65535.0,
                    1.0,
                ],
                ElementShape::Vec4(ElementType::U16) => [
                    elem.read_u16() as f32 / 65535.0,
                    elem.read_u16() as f32 / 65535.0,
                    elem.read_u16() as f32 / 65535.0,
                    elem.read_u16() as f32 / 65535.0,
                ],

                _ => unreachable!(),
            }
        }
    }
}
