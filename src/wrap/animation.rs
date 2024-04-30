//! Structures for glTF animation

use std::borrow::Cow;

use super::{Accessor, Document, Node};
use crate::error::Result;
use bevy::{
    animation::{AnimationClip, EntityPath, Interpolation, Keyframes, VariableCurve},
    asset::LoadContext,
    core::Name,
    math::{Quat, Vec3},
};
use gltf::animation::Property;
use iter::{Channels, Samplers};

/// A glTF animation description
pub struct Animation<'a> {
    doc: Document<'a>,
    raw: gltf::Animation<'a>,
}

impl<'a> Animation<'a> {
    pub(crate) fn new(doc: Document<'a>, raw: gltf::Animation<'a>) -> Self {
        Self { doc, raw }
    }

    /// The raw glTF index of this animation
    #[inline(always)]
    pub fn index(&self) -> usize {
        self.raw.index()
    }

    /// Returns the optional user-defined name
    pub fn name(&self) -> Option<&'a str> {
        self.raw.name()
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

    /// Loads this animation as a bevy [AnimationClip]
    pub async fn load_animation_clip(&self, ctx: &mut LoadContext<'_>) -> Result<AnimationClip> {
        let mut clip = AnimationClip::default();

        for channel in self.channels() {
            let curve = channel.load_variable_curve(ctx).await?;
            let parts = channel
                .node()
                .path()
                .iter()
                .map(|s| Name::new(Cow::Owned(s.clone())))
                .collect();
            clip.add_curve_to_path(EntityPath { parts }, curve);
        }

        Ok(clip)
    }
}

/// Animation sampler data, provides input (time) and output (property) data
pub struct Sampler<'a> {
    doc: Document<'a>,
    raw: gltf::animation::Sampler<'a>,
}

impl<'a> Sampler<'a> {
    /// Returns the [Accessor] containing the input values (e.g. Time)
    pub fn input(&self) -> Accessor<'a> {
        Accessor::new(self.doc, self.raw.input())
    }

    /// Reutrns the [Accessor] containing the target property values
    pub fn output(&self) -> Accessor<'a> {
        Accessor::new(self.doc, self.raw.output())
    }

    /// Returns the [Interpolation] method between two keyframes
    pub fn interpolation(&self) -> Interpolation {
        match self.raw.interpolation() {
            gltf::animation::Interpolation::CubicSpline => Interpolation::CubicSpline,
            gltf::animation::Interpolation::Linear => Interpolation::Linear,
            gltf::animation::Interpolation::Step => Interpolation::Step,
        }
    }
}

/// Targets a [Sampler] to a particular property of a [Node]
pub struct Channel<'a> {
    doc: Document<'a>,
    raw: gltf::animation::Channel<'a>,
}

impl<'a> Channel<'a> {
    /// Returns the parent [Animation]
    pub fn animation(&self) -> Animation<'a> {
        Animation {
            doc: self.doc,
            raw: self.raw.animation(),
        }
    }

    /// Returns the [Sampler] for this channel
    pub fn sampler(&self) -> Sampler<'a> {
        Sampler {
            doc: self.doc,
            raw: self.raw.sampler(),
        }
    }

    /// Returns the target [Node] of this channel
    pub fn node(&self) -> Node<'a> {
        self.target().node()
    }

    /// Returns the target [Property] of this channel
    #[inline(always)]
    pub fn property(&self) -> Property {
        self.raw.target().property()
    }

    /// Returns the node and property to target
    pub fn target(&self) -> Target<'a> {
        Target {
            doc: self.doc,
            raw: self.raw.target(),
        }
    }

    /// Load a bevy [VariableCurve] from this animation channel
    pub async fn load_variable_curve(&self, ctx: &mut LoadContext<'_>) -> Result<VariableCurve> {
        let sampler = self.sampler();

        Ok(VariableCurve {
            keyframe_timestamps: sampler.input().load::<f32>(ctx).await?.iter().collect(),
            keyframes: {
                let output = sampler.output().load_untyped(ctx).await?;
                match self.property() {
                    Property::Translation => {
                        Keyframes::Translation(output.try_with_type::<Vec3>()?.iter().collect())
                    }
                    Property::Rotation => {
                        Keyframes::Rotation(output.try_with_type::<Quat>()?.iter().collect())
                    }
                    Property::Scale => {
                        Keyframes::Scale(output.try_with_type::<Vec3>()?.iter().collect())
                    }
                    Property::MorphTargetWeights => {
                        Keyframes::Weights(output.try_with_type::<f32>()?.iter().collect())
                    }
                }
            },
            interpolation: sampler.interpolation(),
        })
    }
}

/// Information about the target [Node] and [Property] for an animation [Channel]
pub struct Target<'a> {
    doc: Document<'a>,
    raw: gltf::animation::Target<'a>,
}

impl<'a> Target<'a> {
    /// Returns the parent [Animation]
    pub fn animation(&self) -> Animation<'a> {
        Animation {
            doc: self.doc,
            raw: self.raw.animation(),
        }
    }

    /// Returns the target [Node]
    pub fn node(&self) -> Node<'a> {
        Node::new(self.doc, self.raw.node())
    }

    /// Returns the target [Property]
    pub fn property(&self) -> Property {
        self.raw.property()
    }
}

/// Iterators for [Animation] items
pub mod iter {
    use super::{Channel, Sampler};
    use crate::Document;

    /// Iterator over [Sampler]s in an animation
    pub struct Samplers<'a> {
        pub(super) doc: Document<'a>,
        pub(super) raw: gltf::animation::iter::Samplers<'a>,
    }

    impl<'a> Iterator for Samplers<'a> {
        type Item = Sampler<'a>;

        fn next(&mut self) -> Option<Self::Item> {
            self.raw.next().map(|raw| Sampler { doc: self.doc, raw })
        }
    }

    /// Iterator over [Channel]s in an animation
    pub struct Channels<'a> {
        pub(super) doc: Document<'a>,
        pub(super) raw: gltf::animation::iter::Channels<'a>,
    }

    impl<'a> Iterator for Channels<'a> {
        type Item = Channel<'a>;

        fn next(&mut self) -> Option<Self::Item> {
            self.raw.next().map(|raw| Channel { doc: self.doc, raw })
        }
    }
}
