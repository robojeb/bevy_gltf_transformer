//! Wrapper around [gltf::Document] to provide easy access to [bevy] types
//! corresponding to glTF data.
//!
//! This wrapper also caches the loaded glTF buffer data.
pub mod accessor;
#[cfg(feature = "animation")]
pub mod animation;
pub mod buffer;
#[cfg(feature = "gltf_lights")]
pub mod light;
pub mod material;
pub mod mesh;
pub mod scene;
pub mod texture;

use std::sync::OnceLock;

pub use accessor::{Accessor, ElementShape, ElementType, Indices, Values};
use bevy::utils::HashMap;
pub use buffer::{Buffer, View};
#[cfg(feature = "gltf_lights")]
pub use light::Light;
pub use material::Material;
pub use mesh::{Mesh, Primitive};
pub use scene::{Node, Scene};
pub use texture::{Image, Sampler, Texture};

use crate::util::Cache;

const URI_ERROR: &str = "URI Contained invalid percent encoding";
const VALID_MIME_TYPES: &[&str] = &["application/octet-stream", "application/gltf-buffer"];

/// Buffer ID for the [Document] cache
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum BufferId {
    /// Data buffer from the data chunk of a `.glb` file
    Bin,
    /// Externally referenced buffer indexed by JSON index
    Buffer(usize),
}

/// A wrapper around a [gltf::Document] which provides conversions to
/// Bevy types and keeps an internal cache of the loaded [Buffer] data.
#[derive(Clone, Copy)]
pub struct Document<'a> {
    pub(crate) inner: &'a DocInner,
}

pub(crate) struct DocInner {
    pub(crate) doc: gltf::Document,
    pub(crate) cache: Cache,
    pub(crate) paths: OnceLock<HashMap<usize, Vec<String>>>,
}

impl<'a> Document<'a> {
    /// Returns the optionally defined default [Scene] for this glTF asset.
    pub fn default_scene(&self) -> Option<Scene<'a>> {
        self.inner.doc.default_scene().map(|s| Scene::new(*self, s))
    }

    /// Returns an [Iterator] that visits the buffers of the glTF asset.
    pub fn buffers(&self) -> iter::Buffers<'a> {
        iter::Buffers::new(*self, self.inner.doc.buffers())
    }

    /// Returns an [Iterator] that visits the buffer views of the glTF asset.
    pub fn views(&self) -> iter::Views<'a> {
        iter::Views::new(*self, self.inner.doc.views())
    }

    /// Returns an [Iterator] that visits the accessors of the glTF asset.
    pub fn accessors(&self) -> iter::Accessors<'a> {
        iter::Accessors::new(*self, self.inner.doc.accessors())
    }

    /// Returns an [Iterator] that visits the materials of the glTF asset.
    pub fn materials(&self) -> iter::Materials<'a> {
        iter::Materials::new(*self, self.inner.doc.materials())
    }

    /// Returns an [Iterator] over the images of the glTF asset.
    pub fn images(&self) -> iter::Images<'a> {
        iter::Images::new(*self, self.inner.doc.images())
    }

    /// Returns an [Iterator] over the textures of the glTF asset.
    pub fn textures(&self) -> iter::Textures<'a> {
        iter::Textures::new(*self, self.inner.doc.textures())
    }

    /// Returns an [Iterator] over all of the samplers of this glTF asset.
    pub fn samplers(&self) -> iter::Samplers<'a> {
        iter::Samplers::new(*self, self.inner.doc.samplers())
    }

    /// Returns an [Iterator] over all of the meshes of this glTF asset.
    pub fn meshes(&self) -> iter::Meshes<'a> {
        iter::Meshes::new(*self, self.inner.doc.meshes())
    }

    /// Returns an [Iterator] over all of the lights in tihs glTF asset.
    #[cfg(feature = "gltf_lights")]
    pub fn lights(&self) -> iter::Lights<'a> {
        iter::Lights::new(*self, self.inner.doc.lights().into_iter().flatten())
    }

    /// Returns an [Iterator] over all the scenes in this glTF asset.
    pub fn scenes(&self) -> iter::Scenes<'a> {
        iter::Scenes::new(*self, self.inner.doc.scenes())
    }

    /// Returns an [Iterator] over all of the nodes in this glTF asset.
    pub fn nodes(&self) -> iter::Nodes<'a> {
        iter::Nodes::new(*self, self.inner.doc.nodes())
    }

    /// Get a [Node] by its reported index
    pub fn get_node(&self, index: usize) -> Option<Node<'a>> {
        self.inner
            .doc
            .nodes()
            .nth(index)
            .map(|n| Node::new(*self, n))
    }

    /// Helper function to compute and cache all the node-paths in the glTF file
    pub(crate) fn node_paths(&self) -> &'a HashMap<usize, Vec<String>> {
        self.inner.paths.get_or_init(|| {
            use scene::traversal::{DepthFirst, Traversal};

            // Get all the roots in all scenes
            let roots = self
                .inner
                .doc
                .scenes()
                .flat_map(|s| s.nodes())
                .map(|n| Node::new(*self, n));

            // Output map
            let mut paths = HashMap::with_capacity(self.inner.doc.nodes().len());

            // Construct all the node paths in reverse order
            for node in DepthFirst::new(*self, roots, ()) {
                let name = node
                    .name()
                    .map(String::from)
                    .unwrap_or_else(|| format!("Node{}", node.index()));

                // Insert self into map
                paths.insert(node.index(), vec![name.clone()]);

                // Add our name to all children
                for child in node.children() {
                    paths.get_mut(&child.index()).unwrap().push(name.clone());
                }
            }

            // Reverse all the path ordering
            paths.values_mut().for_each(|path| path.reverse());

            paths
        })
    }
}

/// Iterators for items in the glTF [Document]
pub mod iter {
    use super::Document;

    macro_rules! mk_iter {
        ($i:ident, $f:ident, $t:ident) => {
            use super::$t;

            #[doc = "An iterator over ["]
            #[doc = std::stringify!($t)]
            #[doc = "]s in the [Document]"]
            pub struct $i<'a>(Document<'a>, gltf::iter::$i<'a>);

            impl<'a> $i<'a> {
                pub(crate) fn new(doc: Document<'a>, sub: gltf::iter::$i<'a>) -> Self {
                    Self(doc, sub)
                }
            }

            impl<'a> Iterator for $i<'a> {
                type Item = $t<'a>;

                fn next(&mut self) -> Option<Self::Item> {
                    self.1.next().map(|i| $t::new(self.0, i))
                }
            }

            impl<'a> ExactSizeIterator for $i<'a> {
                fn len(&self) -> usize {
                    self.1.len()
                }
            }
        };
    }

    mk_iter!(Buffers, buffers, Buffer);
    mk_iter!(Views, views, View);
    mk_iter!(Accessors, accessors, Accessor);
    mk_iter!(Materials, materials, Material);
    mk_iter!(Images, images, Image);
    mk_iter!(Textures, textures, Texture);
    mk_iter!(Samplers, samplers, Sampler);
    mk_iter!(Meshes, meshes, Mesh);
    mk_iter!(Nodes, nodes, Node);
    mk_iter!(Scenes, scenes, Scene);

    use super::Primitive;

    /// An iterator over [Primitive]s in a [Mesh]
    pub struct Primitives<'a>(Document<'a>, gltf::mesh::iter::Primitives<'a>);

    impl<'a> Primitives<'a> {
        pub(crate) fn new(doc: Document<'a>, sub: gltf::mesh::iter::Primitives<'a>) -> Self {
            Self(doc, sub)
        }
    }

    impl<'a> Iterator for Primitives<'a> {
        type Item = Primitive<'a>;

        fn next(&mut self) -> Option<Self::Item> {
            self.1.next().map(|i| Primitive::new(self.0, i))
        }
    }

    impl<'a> ExactSizeIterator for Primitives<'a> {
        fn len(&self) -> usize {
            self.1.len()
        }
    }

    /* Types and includes for lights iterator */
    #[cfg(feature = "gltf_lights")]
    use super::Light;
    #[cfg(feature = "gltf_lights")]
    use std::{iter::Flatten, option::IntoIter};
    #[cfg(feature = "gltf_lights")]
    type LightsSub<'a> = Flatten<IntoIter<gltf::iter::Lights<'a>>>;

    /// An iterator over [Light]s in the [Document]
    #[cfg(feature = "gltf_lights")]
    pub struct Lights<'a>(Document<'a>, LightsSub<'a>);

    #[cfg(feature = "gltf_lights")]
    impl<'a> Lights<'a> {
        pub(crate) fn new(doc: Document<'a>, sub: LightsSub<'a>) -> Self {
            Self(doc, sub)
        }
    }

    #[cfg(feature = "gltf_lights")]
    impl<'a> Iterator for Lights<'a> {
        type Item = Light<'a>;

        fn next(&mut self) -> Option<Self::Item> {
            self.1.next().map(|i| Light::new(self.0, i))
        }
    }

    // pub struct Lights<'a>(Document<'a>, gltf::iter::Lights)
}
