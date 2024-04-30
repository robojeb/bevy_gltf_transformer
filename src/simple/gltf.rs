//! glTF Asset data that mirrors the defaults provided by `bevy_gltf`

use crate::wrap::camera::Projection;
#[cfg(feature = "bevy_3d")]
use crate::wrap::light::LightKind;
#[cfg(feature = "animation")]
use bevy::animation::AnimationClip;
use bevy::{
    asset::Asset,
    prelude::{Component, Handle},
    reflect::{Reflect, TypePath},
    scene::Scene,
    transform::components::Transform,
    utils::HashMap,
};

/// Application specific glTF extra data
#[derive(Debug, Clone, Component, Reflect)]
pub struct GltfExtras {
    /// JSON encoded extra data
    pub value: String,
}

/// Loaded glTF assets with custom Mesh and Material types
#[derive(Asset, TypePath)]
pub struct Gltf<Mesh, Mat>
where
    Mesh: Asset,
    Mat: Asset,
{
    /// Loaded glTF scenes
    pub scenes: Vec<Handle<Scene>>,
    /// Named glTF scenes
    pub named_scenes: HashMap<String, Handle<Scene>>,
    /// Loaded glTF Meshes
    pub meshes: Vec<Handle<GltfMesh<Mesh, Mat>>>,
    /// Named glTF Meshes
    pub named_meshes: HashMap<String, Handle<GltfMesh<Mesh, Mat>>>,
    /// Loaded glTF materials
    pub materials: Vec<Handle<Mat>>,
    /// Named glTF materials
    pub named_materials: HashMap<String, Handle<Mat>>,
    /// Loaded glTF nodes
    pub nodes: Vec<Handle<GltfNode<Mesh, Mat>>>,
    /// Named glTF nodes
    pub named_nodes: HashMap<String, Handle<GltfNode<Mesh, Mat>>>,
    /// Optional default scene
    pub default_scene: Option<Handle<Scene>>,
    /// Loaded glTF animations
    #[cfg(feature = "animation")]
    pub animations: Vec<Handle<AnimationClip>>,
    /// Named glTF animations
    #[cfg(feature = "animation")]
    pub named_animations: HashMap<String, Handle<AnimationClip>>,
}

/// A glTF mesh, which may consist of multiple [GltfPrimitive]s and an optional [GltfExtras].
///
/// See [the relevant glTF specification section](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#reference-mesh).
#[derive(Asset, TypePath)]
pub struct GltfMesh<Mesh, Mat>
where
    Mesh: Asset,
    Mat: Asset,
{
    /// The primitives that compose this glTF mesh
    pub primitives: Vec<GltfPrimitive<Mesh, Mat>>,
    /// Optional extra data for this mesh
    pub extras: Option<GltfExtras>,
}

/// Part of a [GltfMesh] that consists of a custom Mesh type, an optional
/// custom material type and [GltfExtras].
#[derive(Asset, TypePath)]
pub struct GltfPrimitive<Mesh, Mat>
where
    Mesh: Asset,
    Mat: Asset,
{
    /// The mesh asset for this primitive
    pub mesh: Handle<Mesh>,
    /// The material to use to render this primitive
    pub material: Option<Handle<Mat>>,
    /// Optional extras for the mesh
    pub extras: Option<GltfExtras>,
    /// Optional extras for the material
    pub mat_extras: Option<GltfExtras>,
}

/// A glTF node with all of its child nodes, its [GltfMesh], [Transform] and an optional [GltfExtras].
///
/// See the [relevant glTF specification section](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#reference-node).
#[derive(Asset, TypePath)]
pub struct GltfNode<Mesh, Mat>
where
    Mesh: Asset,
    Mat: Asset,
{
    /// Direct children of the node
    pub children: Vec<GltfNode<Mesh, Mat>>,
    /// The mesh at this node
    pub mesh: Option<Handle<GltfMesh<Mesh, Mat>>>,
    /// The camera at this node
    pub camera: Option<Projection>,
    /// The light at this node
    #[cfg(feature = "bevy_3d")]
    pub light: Option<LightKind>,
    /// The transform of this node
    pub transform: Transform,
    /// Optional extras for this nodes
    pub extras: Option<GltfExtras>,
}
