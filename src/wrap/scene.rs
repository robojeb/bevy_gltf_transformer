//! Structures for glTF Scenes and Nodes
//!
//! This module also contains structures for traversing [Node] trees.
pub mod traversal;

use self::traversal::Traversal;
use super::Document;
#[cfg(feature = "gltf_lights")]
use super::Light;
use bevy::{math::Mat4, transform::components::Transform};
use serde_json::{value::RawValue, Value};

/// A glTF scene which defines the root of one or more [Node] trees
#[derive(Clone)]
pub struct Scene<'a> {
    doc: Document<'a>,
    raw: gltf::Scene<'a>,
}

impl<'a> Scene<'a> {
    pub(crate) fn new(doc: Document<'a>, raw: gltf::Scene<'a>) -> Self {
        Self { doc, raw }
    }

    /// Returns the internal glTF index
    #[inline(always)]
    pub fn index(&self) -> usize {
        self.raw.index()
    }

    /// Returns the optional user-defined name for this object
    #[inline(always)]
    pub fn name(&self) -> Option<&'a str> {
        self.raw.name()
    }

    /// Returns an iterator over all of the root [Node]s in this [Scene]
    pub fn nodes(&self) -> RootNodes<'a> {
        RootNodes(self.doc, self.raw.nodes())
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

    /// Perform a traversal over the [Node]s of a scene.
    pub fn walk_nodes<T>(&self) -> T
    where
        T: Traversal<'a>,
    {
        T::new(self.doc, self.nodes(), T::Settings::default())
    }

    /// Perform a traversal over the [Node]s of a scene with explicit traversal
    /// settings.
    pub fn walk_nodes_with_settings<T>(&self, settings: T::Settings) -> T
    where
        T: Traversal<'a>,
    {
        T::new(self.doc, self.nodes(), settings)
    }
}

/// A node in a glTF [Scene] that defines the transform of objects like: Meshes,
/// Lights, and Cameras
#[derive(Clone)]
pub struct Node<'a> {
    doc: Document<'a>,
    raw: gltf::Node<'a>,
}

impl<'a> Node<'a> {
    pub(crate) fn new(doc: Document<'a>, raw: gltf::Node<'a>) -> Self {
        Self { doc, raw }
    }

    /// The raw glTF index of this [Node]
    #[inline(always)]
    pub fn index(&self) -> usize {
        self.raw.index()
    }

    /// Returns the [Node]'s [Transform]
    #[inline]
    pub fn transform(&self) -> Transform {
        let matrix = self.raw.transform().matrix();
        Transform::from_matrix(Mat4::from_cols_array_2d(&matrix))
    }

    /// Returns the [Light] at this [Node]
    #[cfg(feature = "gltf_lights")]
    pub fn light(&self) -> Option<Light<'a>> {
        self.raw.light().map(|l| Light::new(self.doc, l))
    }

    /// Returns an iterator over the children of this [Node]
    pub fn children(&self) -> Children {
        Children(self.doc, self.raw.children())
    }

    /// Perform a traversal over the [Node]s of a scene.
    pub fn walk_nodes<T>(&self) -> T
    where
        T: Traversal<'a>,
    {
        T::new(
            self.doc,
            Some(self.clone()).into_iter(),
            T::Settings::default(),
        )
    }

    /// Perform a traversal over the [Node]s of a scene with explicit traversal
    /// settings.
    pub fn walk_nodes_with_settings<T>(&self, settings: T::Settings) -> T
    where
        T: Traversal<'a>,
    {
        T::new(self.doc, Some(self.clone()).into_iter(), settings)
    }
}

/// An iterator over root nodes in a [Scene]
pub struct RootNodes<'a>(Document<'a>, gltf::scene::iter::Nodes<'a>);

impl<'a> Iterator for RootNodes<'a> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.1.next().map(|n| Node::new(self.0, n))
    }
}

impl<'a> ExactSizeIterator for RootNodes<'a> {
    fn len(&self) -> usize {
        self.1.len()
    }
}

/// An iterator over the children of a [Node]
pub struct Children<'a>(Document<'a>, gltf::scene::iter::Children<'a>);

impl<'a> Iterator for Children<'a> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.1.next().map(|n| Node::new(self.0, n))
    }
}

impl<'a> ExactSizeIterator for Children<'a> {
    fn len(&self) -> usize {
        self.1.len()
    }
}
