//! Structures for glTF animation

use super::Document;

/// A glTF animation description
pub struct Animation<'a> {
    doc: Document<'a>,
    raw: gltf::Animation<'a>,
}

impl<'a> Animation<'a> {
    /// The raw glTF index of this animation
    #[inline(always)]
    pub fn index(&self) -> usize {
        self.raw.index()
    }

    /// An iterator over the animation channels
    pub fn channels(&self) -> Channels<'a> {
        Channels {
            doc: self.doc,
            raw: self.raw.channels(),
        }
    }

    /// An iterator over the animation samplers
    pub fn samplers(&self) -> Samplers<'a> {
        Samplers {
            doc: self.doc,
            raw: self.raw.samplers(),
        }
    }
}

pub struct Sampler<'a> {
    doc: Document<'a>,
    raw: gltf::animation::Sampler<'a>,
}

pub struct Samplers<'a> {
    doc: Document<'a>,
    raw: gltf::animation::iter::Samplers<'a>,
}

pub struct Channel<'a> {
    doc: Document<'a>,
    raw: gltf::animation::Channel<'a>,
}

pub struct Channels<'a> {
    doc: Document<'a>,
    raw: gltf::animation::iter::Channels<'a>,
}
