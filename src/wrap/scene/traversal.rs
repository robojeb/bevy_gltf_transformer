//! Algorithms for traversing a [Node] tree for a [Scene] or [Node]
use super::{Node, Scene};
use crate::wrap::Document;

/// Defines a strategy to traverse over a [Node] tree
pub trait Traversal<'a>: Iterator<Item = Node<'a>> {
    /// Settings that may affect the traversal of the tree
    type Settings: Default + 'a;

    /// Generate a new traversal iterator given the provided root nodes
    fn new(
        document: Document<'a>,
        roots: impl Iterator<Item = Node<'a>>,
        settings: Self::Settings,
    ) -> Self;
}

/// Performs a depth-first traversal of the [Node] tree.
///
/// This traversal returns the `depth` of the node in the tree as
/// [Traversal::ExtData].
///
/// All children of a [Node] will be produced before the parent.
/// [Node]s may be produced multiple times if they appear multiple times in
/// the tree (e.g. for instances).
pub struct DepthFirst<'a> {
    doc: Document<'a>,
    stack: Vec<(usize, usize)>,
}

impl<'a> Traversal<'a> for DepthFirst<'a> {
    type Settings = ();

    fn new(
        doc: Document<'a>,
        roots: impl Iterator<Item = Node<'a>>,
        _settings: Self::Settings,
    ) -> Self {
        Self {
            doc,
            stack: roots.map(|node| (node.index(), 0)).collect(),
        }
    }
}

impl<'a> Iterator for DepthFirst<'a> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((node_idx, child_offset)) = self.stack.last_mut() {
            let node = self.doc.get_node(*node_idx).unwrap();

            // Finished iterating over this Node's children so return it now
            if *child_offset >= node.children().len() {
                self.stack.pop();
                return Some(node);
            }

            // Get the next child
            let child = node.children().nth(*child_offset).unwrap().index();

            // Increment child counter
            *child_offset += 1;

            // Append the next child
            self.stack.push((child, 0))
        }

        None
    }
}

pub(crate) struct FilteredDepthFirst<'a, 'f> {
    doc: Document<'a>,
    filter: &'f dyn Fn(Scene<'a>, Node<'a>) -> bool,
    scene: Scene<'a>,
    stack: Vec<(usize, usize)>,
}

impl<'a, 'f> FilteredDepthFirst<'a, 'f> {
    pub(crate) fn new(
        doc: Document<'a>,
        roots: impl Iterator<Item = Node<'a>>,
        scene: Scene<'a>,
        filter: &'f dyn Fn(Scene<'a>, Node<'a>) -> bool,
    ) -> Self {
        Self {
            doc,
            scene,
            filter,
            stack: roots.map(|node| (node.index(), 0)).collect(),
        }
    }
}

impl<'a> Iterator for FilteredDepthFirst<'a, '_> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        'descend: while let Some((node_idx, child_offset)) = self.stack.last_mut() {
            let node = self.doc.get_node(*node_idx).unwrap();

            while *child_offset < node.children().len() {
                // Get the next child
                let child_node = node.children().nth(*child_offset).unwrap();
                let child = child_node.index();

                // Increment child counter
                *child_offset += 1;

                if (self.filter)(self.scene.clone(), child_node) {
                    // Append the next child
                    self.stack.push((child, 0));
                    // Descend into this child
                    continue 'descend;
                }
            }

            // Finished iterating over this Node's children so return it now
            if *child_offset >= node.children().len() {
                self.stack.pop();
                return Some(node);
            }
        }

        None
    }
}
