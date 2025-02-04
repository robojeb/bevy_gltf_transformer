//! Structures for defining skinned meshes
//!
use super::{Accessor, Document, Node};

/// Joints and inverse bind matrices for skinned meshes
pub struct Skin<'a> {
    doc: Document<'a>,
    raw: gltf::Skin<'a>,
}

impl<'a> Skin<'a> {
    pub(crate) fn new(doc: Document<'a>, raw: gltf::Skin<'a>) -> Self {
        Self { doc, raw }
    }

    /// Returns the raw glTF index
    pub fn index(&self) -> usize {
        self.raw.index()
    }

    /// Returns the optional user-defined name
    pub fn name(&self) -> Option<&'a str> {
        self.raw.name()
    }

    /// Returns the accessor for the inverse bind matrices
    ///
    /// The accessor is expected to contain 4x4 f32 matrices.
    ///
    /// When this returns [None] it is assumed that each matrix is the
    /// identity matrix.
    pub fn inverse_bind_matrices(&self) -> Option<Accessor<'a>> {
        self.raw
            .inverse_bind_matrices()
            .map(|a| Accessor::new(self.doc, a))
    }

    /// Returns the [Node] that is used as the root of the skeleton
    ///
    /// When [None] the joint transforms are relative to the scene root.
    pub fn skeleton(&self) -> Option<Node<'a>> {
        self.raw.skeleton().map(|n| Node::new(self.doc, n))
    }

    /// Returns an iterator over the [Node]s used as joints in this skin
    pub fn joints(&self) -> Joints<'a> {
        Joints {
            doc: self.doc,
            raw: self.raw.joints(),
        }
    }
}

/// An [Iterator] over [Node]s that are used as joints in a skinned mesh
pub struct Joints<'a> {
    doc: Document<'a>,
    raw: gltf::skin::iter::Joints<'a>,
}

impl<'a> Iterator for Joints<'a> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.raw.next().map(|n| Node::new(self.doc, n))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.raw.len(), Some(self.raw.len()))
    }
}

impl ExactSizeIterator for Joints<'_> {}
