//! Structures for glTF Mesh and Primitive data
//!
use self::iter::MorphTargets;

use super::{iter::Primitives, Accessor, Document, ElementShape, ElementType, Material};
use crate::{
    data::DataIter,
    error::{Error, Result},
};
use bevy::{
    asset::LoadContext,
    math::{bounding::Aabb3d, f32::Vec3},
    render::{
        mesh::{
            morph::{MorphAttributes, MorphTargetImage},
            Indices, Mesh as BevyMesh, PrimitiveTopology, VertexAttributeValues,
        },
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
    doc: Document<'a>,
    raw: gltf::Primitive<'a>,
}

impl<'a> Primitive<'a> {
    pub(crate) fn new(doc: Document<'a>, raw: gltf::Primitive<'a>) -> Self {
        Self { doc, raw }
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
        Material::new(self.doc, self.raw.material())
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
        self.raw.get(semantic).map(|a| Accessor::new(self.doc, a))
    }

    /// Returns an iterator over the all the [MorphTarget]s for this primitive
    pub fn morph_targets(&self) -> MorphTargets<'a> {
        iter::MorphTargets {
            doc: self.doc,
            raw: self.raw.morph_targets(),
        }
    }

    /// Loads all of the [MorphTarget]s for this primitive into a single
    /// [MorphTargetImage]
    pub async fn load_morph_image(
        &self,
        ctx: &mut LoadContext<'_>,
        asset_usage: RenderAssetUsages,
    ) -> Result<MorphTargetImage> {
        let mut iters: Vec<MorphAttributeIter> = Vec::new();

        for target in self.morph_targets() {
            iters.push(target.load_morph_attributes(ctx).await?);
        }

        Ok(MorphTargetImage::new(
            iters.into_iter(),
            self.vertex_count()?,
            asset_usage,
        )?)
    }

    /// Loads this primitive as a standard 3D Bevy [Mesh](BevyMesh)
    ///
    /// This will load the following attributes if present:
    ///  * [ATTRIBUTE_POSITION](BevyMesh::ATTRIBUTE_POSITION) using conversion from [attributes::AttrPosition]
    ///  * [ATTRIBUTE_NORMAL](BevyMesh::ATTRIBUTE_NORMAL) only for `[f32;3]` accessors
    ///  * [ATTRIBUTE_TANGENT](BevyMesh::ATTRIBUTE_TANGENT)  only for `[f32;3]` accessors
    ///  * [ATTRIBUTE_UV_0](BevyMesh::ATTRIBUTE_UV_0) and [ATTRIBUTE_UV_1](BevyMesh::ATTRIBUTE_UV_1) using conversions from [attributes::AttrTexCoord]
    ///  * [ATTRIBUTE_COLOR](BevyMesh::ATTRIBUTE_COLOR) using conversions  from [attributes::AttrColor]
    ///  * [ATTRIBUTE_JOINT_INDEX](BevyMesh::ATTRIBUTE_JOINT_INDEX) using conversions from [attributes::AttrJointIndex]
    ///  * [ATTRIBUTE_JOINT_WEIGHT](BevyMesh::ATTRIBUTE_JOINT_WEIGHT) using conversions from [attributes::AttrJointWeight]
    ///
    /// If any of the underlying accessors is missing or the incorrect type
    /// to be converted, it will be skipped. Any other errors while loading
    /// accessor data will cause the function to return an error.
    ///
    /// NOTE: The conversions do not take into account the [Accessor]'s
    /// normalization status. So for joint weights in the standard bevy loader
    /// a `[u8; 2]` normalized accessor would be accepted, but an unnormalized
    /// accessor would become an error.
    pub async fn as_mesh(
        &self,
        ctx: &mut LoadContext<'_>,
        asset_usage: RenderAssetUsages,
    ) -> Result<BevyMesh> {
        let mut mesh = BevyMesh::new(self.topology()?, asset_usage);

        // Helper macro to filter out accessor type issues and skip those
        // attributes
        macro_rules! check_accessor {
            ($accessor:ident.load::<$attr:ty>($ctx:ident)) => {
                match $accessor.load::<$attr>($ctx).await {
                    Ok(x) => x.iter().collect(),
                    Err(Error::AccessorType { .. }) => continue,
                    Err(e) => return Err(e),
                }
            };
        }

        for (attr, raw_accessor) in self.raw.attributes() {
            let accessor = Accessor::new(self.doc, raw_accessor);

            let (attr, value) =
                match attr {
                    Semantic::Positions => (
                        BevyMesh::ATTRIBUTE_POSITION,
                        VertexAttributeValues::Float32x3(check_accessor!(accessor
                            .load::<attributes::AttrPosition>(
                            ctx
                        ))),
                    ),
                    Semantic::Normals => (
                        BevyMesh::ATTRIBUTE_NORMAL,
                        VertexAttributeValues::Float32x3(check_accessor!(
                            accessor.load::<[f32; 3]>(ctx)
                        )),
                    ),
                    Semantic::Tangents => (
                        BevyMesh::ATTRIBUTE_TANGENT,
                        VertexAttributeValues::Float32x3(check_accessor!(
                            accessor.load::<[f32; 3]>(ctx)
                        )),
                    ),
                    Semantic::TexCoords(c) if (0..=1).contains(&c) => (
                        match c {
                            0 => BevyMesh::ATTRIBUTE_UV_0,
                            1 => BevyMesh::ATTRIBUTE_UV_1,
                            _ => unreachable!(),
                        },
                        VertexAttributeValues::Float32x2(check_accessor!(accessor
                            .load::<attributes::AttrTexCoord>(
                            ctx
                        ))),
                    ),
                    Semantic::Colors(0) => (
                        BevyMesh::ATTRIBUTE_COLOR,
                        VertexAttributeValues::Float32x4(check_accessor!(accessor
                            .load::<attributes::AttrColor>(
                            ctx
                        ))),
                    ),
                    Semantic::Joints(0) => (
                        BevyMesh::ATTRIBUTE_JOINT_INDEX,
                        VertexAttributeValues::Uint16x4(check_accessor!(
                            accessor.load::<attributes::AttrJointIndex>(ctx)
                        )),
                    ),
                    Semantic::Weights(0) => (
                        BevyMesh::ATTRIBUTE_JOINT_WEIGHT,
                        VertexAttributeValues::Float32x4(check_accessor!(
                            accessor.load::<attributes::AttrJointWeight>(ctx)
                        )),
                    ),
                    _ => continue,
                };

            mesh.insert_attribute(attr, value);
        }

        if let Some(raw_index_accessor) = self.raw.indices() {
            let indices = Accessor::new(self.doc, raw_index_accessor);

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

    /// Attempt to get the vertex count be inspecting the positions, normals, or
    /// tangents (in that order) of this primitive.
    fn vertex_count(&self) -> Result<usize> {
        Ok(self
            .get_accessor(&Semantic::Positions)
            .or_else(|| self.get_accessor(&Semantic::Normals))
            .or_else(|| self.get_accessor(&Semantic::Tangents))
            .ok_or(Error::PrimitiveVertexCount)?
            .len())
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

/// A single morph target for a primitive
pub struct MorphTarget<'a> {
    doc: Document<'a>,
    raw: gltf::mesh::MorphTarget<'a>,
}

impl<'a> MorphTarget<'a> {
    /// Returns the vertex position offsets XYZ
    pub fn positions(&self) -> Option<Accessor<'a>> {
        self.raw.positions().map(|a| Accessor::new(self.doc, a))
    }

    /// Returns the vertex normal offsets XYZ
    pub fn normals(&self) -> Option<Accessor<'a>> {
        self.raw.normals().map(|a| Accessor::new(self.doc, a))
    }

    /// Returns the vertex tangent offsets XYZ
    pub fn tangents(&self) -> Option<Accessor<'a>> {
        self.raw.tangents().map(|a| Accessor::new(self.doc, a))
    }

    /// Loads the data for this [MorphTarget] and produces an iterator over
    /// the [MorphAttributes] data
    pub async fn load_morph_attributes(
        &self,
        ctx: &mut LoadContext<'_>,
    ) -> Result<MorphAttributeIter<'a>> {
        let positions = if let Some(positions) = self.positions() {
            Some(positions.load::<Vec3>(ctx).await?.iter())
        } else {
            None
        };

        let normals = if let Some(normals) = self.normals() {
            Some(normals.load::<Vec3>(ctx).await?.iter())
        } else {
            None
        };

        let tangents = if let Some(tangents) = self.tangents() {
            Some(tangents.load::<Vec3>(ctx).await?.iter())
        } else {
            None
        };

        Ok(MorphAttributeIter {
            positions,
            normals,
            tangents,
        })
    }

    /// Load this [MorphTarget] as a bevy [MorphTargetImage]
    pub async fn load_morph_image(
        &self,
        ctx: &mut LoadContext<'_>,
        vertex_count: usize,
        asset_usage: RenderAssetUsages,
    ) -> Result<MorphTargetImage> {
        Ok(MorphTargetImage::new(
            std::iter::once(self.load_morph_attributes(ctx).await?),
            vertex_count,
            asset_usage,
        )?)
    }
}

/// An [Iterator] over morph target attribute data
///
/// This iterator will infinitely return values replacing any missing values
/// (either due to an underlying accessor running out of data, or one of the
/// accessors not being provided) with [Vec3::ZERO].
pub struct MorphAttributeIter<'a> {
    positions: Option<DataIter<'a, Vec3>>,
    normals: Option<DataIter<'a, Vec3>>,
    tangents: Option<DataIter<'a, Vec3>>,
}

impl<'a> Iterator for MorphAttributeIter<'a> {
    type Item = MorphAttributes;

    fn next(&mut self) -> Option<Self::Item> {
        Some(MorphAttributes {
            position: self
                .positions
                .as_mut()
                .and_then(|p| p.next())
                .unwrap_or(Vec3::ZERO),
            normal: self
                .normals
                .as_mut()
                .and_then(|p| p.next())
                .unwrap_or(Vec3::ZERO),
            tangent: self
                .tangents
                .as_mut()
                .and_then(|p| p.next())
                .unwrap_or(Vec3::ZERO),
        })
    }
}

/// Accessors for reading mesh vertex attributes
pub mod attributes {

    use crate::{
        data::Accessible,
        util::norm::Normalizable,
        wrap::{ElementShape, ElementType},
    };

    /// Reads accessor data into values for `Mesh::ATTRIBUTE_COLOR`
    ///     
    /// ## Conversions
    ///
    /// * `data: [f32; 4] => data`
    /// * `data: [f32; 3] => [data[0], data[1], data[2], 1.0]`
    /// * `data: [u16; 4] => norm(data)`
    /// * `data: [u16; 3] => [norm(data[0]), norm(data[1]), norm(data[2]), 1.0]`
    /// * `data: [u8; 4]  => norm(data)`
    /// * `data: [u8; 3]  => [norm(data[0]), norm(data[1]), norm(data[2]), 1.0]`
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
                    elem.read_u8().norm(),
                    elem.read_u8().norm(),
                    elem.read_u8().norm(),
                    1.0,
                ],
                ElementShape::Vec4(ElementType::U8) => [
                    elem.read_u8().norm(),
                    elem.read_u8().norm(),
                    elem.read_u8().norm(),
                    elem.read_u8().norm(),
                ],

                ElementShape::Vec3(ElementType::U16) => [
                    elem.read_u16().norm(),
                    elem.read_u16().norm(),
                    elem.read_u16().norm(),
                    1.0,
                ],
                ElementShape::Vec4(ElementType::U16) => [
                    elem.read_u16().norm(),
                    elem.read_u16().norm(),
                    elem.read_u16().norm(),
                    elem.read_u16().norm(),
                ],

                _ => unreachable!(),
            }
        }
    }

    /// Reads accessor values appropriate for `Mesh::ATTRIBUTE_POSITION`
    ///
    /// ## Conversions
    ///
    /// * `data: [f32; 3] => data`
    /// * `data: [f32; 2] => [data[0], 0.0, data[1]]`
    pub struct AttrPosition;

    impl Accessible for AttrPosition {
        type Item = [f32; 3];

        fn from_element(mut elem: crate::data::Element) -> Self::Item {
            match elem.shape {
                ElementShape::Vec2(ElementType::F32) => [elem.read_f32(), 0.0, elem.read_f32()],
                ElementShape::Vec3(ElementType::F32) => {
                    [elem.read_f32(), elem.read_f32(), elem.read_f32()]
                }
                _ => unreachable!(),
            }
        }

        fn validate_accessor(shape: ElementShape) -> bool {
            matches!(
                shape,
                ElementShape::Vec2(ElementType::F32) | ElementShape::Vec3(ElementType::F32)
            )
        }

        fn zero(_shape: ElementShape) -> Self::Item {
            [0.0, 0.0, 0.0]
        }
    }

    /// Reads accessor values appropriate for `Mesh::ATTRIBUTE_UV{0,1}`
    ///
    /// ## Conversions
    ///
    /// * `data: [f32; 2] => data`
    /// * `data: [u16; 2] => [norm(data[0]), norm(data[1])]`
    /// * `data: [u8; 2] => [norm(data[0]), norm(data[1])]`
    pub struct AttrTexCoord;

    impl Accessible for AttrTexCoord {
        type Item = [f32; 2];

        fn zero(_shape: ElementShape) -> Self::Item {
            [0.0, 0.0]
        }

        fn validate_accessor(shape: ElementShape) -> bool {
            matches!(
                shape,
                ElementShape::Vec2(ElementType::F32 | ElementType::U16 | ElementType::U8)
            )
        }

        fn from_element(mut elem: crate::data::Element) -> Self::Item {
            match elem.shape {
                ElementShape::Vec2(ElementType::F32) => [elem.read_f32(), elem.read_f32()],
                ElementShape::Vec2(ElementType::U16) => {
                    [elem.read_u16().norm(), elem.read_u16().norm()]
                }
                ElementShape::Vec2(ElementType::U8) => {
                    [elem.read_u8().norm(), elem.read_u8().norm()]
                }
                _ => unreachable!(),
            }
        }
    }

    /// Reads accessor values appropriate for `Mesh::ATTRIBUTE_JOINT_INDEX`
    ///
    /// ## Conversions
    ///
    /// * `data: [u16; 4] => data`
    /// * `data: [u8; 4] => data as [u16; 4]`
    pub struct AttrJointIndex;

    impl Accessible for AttrJointIndex {
        type Item = [u16; 4];

        fn zero(_shape: ElementShape) -> Self::Item {
            [0; 4]
        }

        fn validate_accessor(shape: ElementShape) -> bool {
            matches!(
                shape,
                ElementShape::Vec4(ElementType::U16 | ElementType::U8)
            )
        }

        fn from_element(mut elem: crate::data::Element) -> Self::Item {
            match elem.shape {
                ElementShape::Vec4(ElementType::U16) => [
                    elem.read_u16(),
                    elem.read_u16(),
                    elem.read_u16(),
                    elem.read_u16(),
                ],
                ElementShape::Vec4(ElementType::U8) => [
                    elem.read_u8() as u16,
                    elem.read_u8() as u16,
                    elem.read_u8() as u16,
                    elem.read_u8() as u16,
                ],
                _ => unreachable!(),
            }
        }
    }

    /// Reads accessor data into values for `Mesh::ATTRIBUTE_JOINT_WEIGHT`
    ///
    /// ## Conversions
    ///
    /// * `data: [f32; 4] => data`
    /// * `data: [u16; 4] => norm(data)`
    /// * `data: [u8; 4]  => norm(data)`
    pub struct AttrJointWeight;

    impl Accessible for AttrJointWeight {
        type Item = [f32; 4];

        fn validate_accessor(shape: crate::wrap::ElementShape) -> bool {
            matches!(
                shape,
                ElementShape::Vec4(ElementType::F32 | ElementType::U16 | ElementType::U8)
            )
        }

        fn zero(_shape: ElementShape) -> Self::Item {
            [0.0; 4]
        }

        fn from_element(mut elem: crate::data::Element) -> Self::Item {
            match elem.shape {
                ElementShape::Vec4(ElementType::F32) => [
                    elem.read_f32(),
                    elem.read_f32(),
                    elem.read_f32(),
                    elem.read_f32(),
                ],

                ElementShape::Vec4(ElementType::U8) => [
                    elem.read_u8().norm(),
                    elem.read_u8().norm(),
                    elem.read_u8().norm(),
                    elem.read_u8().norm(),
                ],

                ElementShape::Vec4(ElementType::U16) => [
                    elem.read_u16().norm(),
                    elem.read_u16().norm(),
                    elem.read_u16().norm(),
                    elem.read_u16().norm(),
                ],

                _ => unreachable!(),
            }
        }
    }
}

/// Iterators for mesh and primitive child items
pub mod iter {
    use super::{Document, MorphTarget};

    /// An [Iterator] over the morph targets of a primitive
    pub struct MorphTargets<'a> {
        pub(super) doc: Document<'a>,
        pub(super) raw: gltf::mesh::iter::MorphTargets<'a>,
    }

    impl<'a> Iterator for MorphTargets<'a> {
        type Item = MorphTarget<'a>;

        fn next(&mut self) -> Option<Self::Item> {
            self.raw.next().map(|m| MorphTarget {
                doc: self.doc,
                raw: m,
            })
        }
    }

    impl<'a> ExactSizeIterator for MorphTargets<'a> {
        fn len(&self) -> usize {
            self.raw.len()
        }
    }
}
