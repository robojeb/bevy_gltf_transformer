//! Structured glTF asset loading with simple transformation of Material and
//! Mesh assets
pub mod gltf;

use bevy::{
    app::Plugin,
    asset::{Asset, AssetApp, Handle, LoadContext},
    ecs::{
        entity::Entity,
        world::{FromWorld, World},
    },
    hierarchy::{BuildChildren, Children},
    log::warn,
    render::view::Visibility,
    scene::Scene as BevyScene,
    tasks::futures_lite::prelude::Future,
    utils::hashbrown::HashMap,
};

use crate::{
    wrap::{scene::traversal::FilteredDepthFirst, Material, Mesh, Node, Primitive, Scene},
    GltfTransformLoader, GltfTransformer,
};

/// Plugin to add a new [SimpleGltfTransformer] and its associated
/// [Gltf](gltf::Gltf) type to an app
pub struct SimpleGltfPlugin<S: SimpleGltfTransformer>(pub S::PluginSettings);

impl<S> Plugin for SimpleGltfPlugin<S>
where
    S: SimpleGltfTransformer,
{
    fn build(&self, app: &mut bevy::prelude::App) {
        app.register_asset_loader(GltfTransformLoader(S::from_plugin(&self.0)))
            .init_asset::<gltf::Gltf<S::Mesh, S::Material>>()
            .init_asset::<gltf::GltfNode<S::Mesh, S::Material>>()
            .init_asset::<gltf::GltfMesh<S::Mesh, S::Material>>()
            .init_asset::<gltf::GltfPrimitive<S::Mesh, S::Material>>();
        // TODO: Systems to allow node and mesh loading?
    }
}

/// A simple interface to implement a [GltfTransformer]
///
/// This allows customizing the Material and Mesh types that get loaded, while
/// reusing common scene loading code.
pub trait SimpleGltfTransformer: Send + Sync + 'static {
    /// The material asset for this transfomer
    type Material: Asset;
    /// The mesh asset for this transformer
    type Mesh: Asset;
    /// Settings provided at plugin time to change the loader behavior
    type PluginSettings: Send + Sync + 'static;
    /// The settings type used by this [`GltfTransformer`].
    type LoadSettings: bevy::asset::meta::Settings
        + Default
        + serde::Serialize
        + for<'a> serde::Deserialize<'a>;
    /// The type of [error](`std::error::Error`) which could be encountered by this transformer.
    type Error: Into<Box<dyn std::error::Error + Send + Sync + 'static>> + From<crate::error::Error>;

    /// Construct a this loader from settings stored in the [SimpleGltfPlugin]
    fn from_plugin(setttings: &Self::PluginSettings) -> Self;

    /// Optionally process a default material for primitives which do not
    /// have a recorded material.
    ///
    /// This is useful for debugging such as producing a vibrant pink material
    /// for missing/unassigned materials.
    ///
    /// The provided [Material] contains glTF supplied default material
    /// properties.
    ///
    /// If no material is returned, the associated [gltf::GltfPrimitive] will
    /// have no `material` specified and may not render in any loaded scenes.
    ///
    /// ### Default Behavior
    /// By default this function will call [SimpleGltfTransformer::process_material]
    /// on the default GLTF material parameters.
    fn default_material<'a>(
        &'a self,
        ctx: &'a mut LoadContext,
        settings: &'a Self::LoadSettings,
        material: Material<'a>,
    ) -> impl Future<Output = Result<Option<Self::Material>, Self::Error>> + Send {
        async {
            let mat = self.process_material(ctx, settings, material).await?;
            Ok(Some(mat))
        }
    }

    /// Process a material node and produce the output material type
    fn process_material<'a>(
        &'a self,
        ctx: &'a mut LoadContext<'_>,
        settings: &'a Self::LoadSettings,
        material: Material<'a>,
    ) -> impl Future<Output = Result<Self::Material, Self::Error>> + Send;

    /// Process a primitive for the given [Mesh]
    ///
    /// This can filter primitives by returning [None] otherwise this must
    /// return the associated [SimpleGltfTransformer::Mesh] type.
    fn process_primitive<'a>(
        &'a self,
        ctx: &'a mut LoadContext<'_>,
        settings: &'a Self::LoadSettings,
        mesh: Mesh<'a>,
        primitive: Primitive<'a>,
    ) -> impl Future<Output = Result<Option<Self::Mesh>, Self::Error>> + Send;

    /// Optionally filters out [Nodes](Node) from a [Scene] tree
    ///
    /// For any [Node] that this function return's `false`, that node and
    /// all of its children will be pruned from the [Scene].
    ///
    /// ### Default Behavior
    /// By default this will return `true` for all nodes.
    fn node_filter(&self, scene: Scene, node: Node) -> bool {
        // Squash lint warnings without changing api names
        let _ = (scene, node);
        true
    }

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

impl<S> GltfTransformer for S
where
    S: SimpleGltfTransformer,
{
    type Asset = gltf::Gltf<S::Mesh, S::Material>;
    type Settings = S::LoadSettings;
    type Error = S::Error;

    /* Loads a [Gltf](gltf::Gltf) asset using the [SimpleGltfTransformer]
     * processing functions.
     *
     * Loading takes several steps:
     *  1. All materials are loaded with [SimpleGltfTransformer::process_material]
     *    * Each material will determine the appropriate settings for textures that
     *      it uses. This may lead to duplicate loading of textures. Future
     *      implementations will provide a cache of preloaded textures.
     *  2. All meshes are loaded and [GltfPrimitive](gltf::GltfPrimitive) assets
     *     are created with the mesh data and associated material. If the glTF
     *     default material is specified, the [SimpleGltfTransformer::default_material]
     *     function will be called and the result will be cached for future use.
     *  3. Scenes will be processed and an entity hierarchy will be constructed.
     *    * Nodes which do not have a user specified name will have a name generated
     *      based on their glTF index, e.g. `Node23`.
     *  4. (Feature "animations" only) Animations will be loaded as
     *     [AnimationClips](bevy::animation::AnimationClip).
     */
    async fn load<'a>(
        &'a self,
        document: crate::wrap::Document<'_>,
        settings: &'a Self::Settings,
        ctx: &'a mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        /*
         * 1) Process materials
         *
         * This may cause duplicate loads of textures, perhaps there is a
         * better way to provide cached textures to the loader.
         */
        // FIXME: Look into texture caching
        let mut materials = Vec::new();
        let mut named_materials = HashMap::new();

        for material in document.materials() {
            let index = material
                .index()
                .expect("Material iterator should not return Default Material");
            let name = material.name();
            let material_asset = self.process_material(ctx, settings, material).await?;
            let handle = ctx.add_labeled_asset(format!("Material{}", index), material_asset);

            materials.push(handle.clone());
            if let Some(name) = name {
                named_materials.insert(String::from(name), handle);
            }
        }

        let mut default_material: Option<Option<_>> = None;

        /*
         * 2) Process Meshes
         */
        let mut meshes = Vec::new();
        let mut named_meshes = HashMap::new();

        for mesh in document.meshes() {
            let mut mesh_ctx = ctx.begin_labeled_asset();
            let mut mesh_asset = gltf::GltfMesh {
                primitives: Vec::new(),
                extras: None,
            };
            let index = mesh.index();
            let name = mesh.name();

            for primitive in mesh.primitives() {
                let prim_index = primitive.index();

                // 2.1) Get the material handle for this primitive
                let mat_handle = if let Some(index) = primitive.material().index() {
                    materials.get(index).cloned()
                } else if let Some(default_mat) = &default_material {
                    default_mat.clone()
                } else {
                    // FIXME: using `mesh_ctx` may cause this default material to have the wrong asset path
                    // Check if a default material is provided here
                    if let Some(material) = self
                        .default_material(&mut mesh_ctx, settings, primitive.material())
                        .await?
                    {
                        let handle =
                            mesh_ctx.add_labeled_asset(String::from("Material/Default"), material);

                        default_material = Some(Some(handle.clone()));

                        Some(handle)
                    } else {
                        default_material = Some(None);
                        None
                    }
                };

                let Some(prim) = self
                    .process_primitive(&mut mesh_ctx, settings, mesh.clone(), primitive)
                    .await?
                else {
                    continue;
                };
                let handle =
                    mesh_ctx.add_labeled_asset(format!("Mesh{index}/Primitive{prim_index}"), prim);

                mesh_asset.primitives.push(gltf::GltfPrimitive {
                    mesh: handle,
                    extras: None,
                    material: mat_handle,
                    mat_extras: None,
                });
            }

            let mesh_asset = mesh_ctx.finish(mesh_asset, None);
            let mesh_handle = ctx.add_loaded_labeled_asset(format!("Mesh{index}"), mesh_asset);

            meshes.push(mesh_handle.clone());
            if let Some(name) = name {
                named_meshes.insert(String::from(name), mesh_handle);
            }
        }

        /*
         * 4) Process animations
         */

        #[cfg(feature = "animation")]
        let (animations, named_animations) = {
            let mut animations = Vec::with_capacity(document.animations().len());
            let mut named_animations = HashMap::new();

            for animation in document.animations() {
                let clip = animation.load_animation_clip(ctx).await?;
                let handle = ctx.add_labeled_asset(format!("Animation{}", animation.index()), clip);

                if let Some(name) = animation.name() {
                    named_animations.insert(String::from(name), handle.clone());
                }
                animations.push(handle);
            }

            Ok((animations, named_animations))
        }?;

        /*
         * 4) Process Scenes
         */
        let nodes = Vec::with_capacity(document.nodes().len());
        let named_nodes = HashMap::new();
        let mut scenes: Vec<Handle<BevyScene>> = Vec::with_capacity(document.scenes().len());
        let mut named_scenes = HashMap::new();

        // Cache entities as we traverse up the tree
        let mut entity_cache: HashMap<usize, Entity> =
            HashMap::with_capacity(document.nodes().len());

        for scene in document.scenes() {
            let mut scene_world = World::new();
            // Reset the entity mapping cache to remove old root-nodes
            entity_cache.clear();

            let filter = |s, n| self.node_filter(s, n);
            let filtered_traversal =
                FilteredDepthFirst::new(document, scene.nodes(), scene.clone(), &filter);

            for node in filtered_traversal {
                // Create child component ahead of time to prevent archetype moves
                let child_component = Children::from_world(&mut scene_world);

                // Spawn the entity with all the components we know for sure
                // will be attached to this node entity.
                let mut node_entity =
                    scene_world.spawn((child_component, node.transform(), Visibility::default()));

                // Attach children
                for child in node.children() {
                    let Some(child_entity) = entity_cache.remove(&child.index()) else {
                        warn!("Missing child entity");
                        continue;
                    };

                    node_entity.add_child(child_entity);
                }

                // Insert into the cache
                entity_cache.insert(node.index(), node_entity.id());
            }

            let scene_asset = BevyScene::new(scene_world);
            let handle = ctx.add_labeled_asset(format!("Scene{}", scene.index()), scene_asset);

            if let Some(name) = scene.name() {
                named_scenes.insert(String::from(name), handle.clone());
            }
            scenes.push(handle);
        }

        Ok(gltf::Gltf {
            default_scene: document
                .default_scene()
                .and_then(|s| scenes.get(s.index()).cloned()),
            scenes,
            named_scenes,
            meshes,
            named_meshes,
            materials,
            named_materials,
            nodes,
            named_nodes,
            #[cfg(feature = "animation")]
            animations,
            #[cfg(feature = "animation")]
            named_animations,
        })
    }

    fn extensions(&self) -> &[&str] {
        <S as SimpleGltfTransformer>::extensions(self)
    }
}
