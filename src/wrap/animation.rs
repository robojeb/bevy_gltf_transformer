//! Structures for glTF animation

use super::{Accessor, Document, Node};
use crate::error::{Error, Result};
use bevy::{
    animation::{
        animated_field,
        gltf_curves::{CubicKeyframeCurve, CubicRotationCurve, SteppedKeyframeCurve},
        prelude::*,
        AnimationClip, AnimationTargetId, VariableCurve,
    },
    asset::LoadContext,
    math::{
        curve::{ConstantCurve, Interval, UnevenSampleAutoCurve},
        Quat, Vec3, Vec4,
    },
    transform::components::Transform,
};
use gltf::animation::{Interpolation, Property};
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

    /// Loads this animation as a bevy [AnimationClip] potentially remapping
    /// the [Channel]s
    ///
    /// The `target_map` parameter will be called for each Animation [Channel]
    /// and should return an appropriate [AnimationTargetId] or [None].
    /// If [None] is returned the [Channel] will not be included in the
    /// [AnimationClip].
    pub async fn load_animation_clip_with_targets<F>(
        &self,
        ctx: &mut LoadContext<'_>,
        mut target_map: F,
    ) -> Result<AnimationClip>
    where
        F: FnMut(&Channel) -> Option<AnimationTargetId>,
    {
        let mut clip = AnimationClip::default();

        for channel in self.channels() {
            // Check if this should be filtered before we spend our time loading it
            if let Some(target_id) = target_map(&channel) {
                let curve = channel.load_variable_curve(ctx).await?;
                clip.add_variable_curve_to_target(target_id, curve);
            }
        }

        Ok(clip)
    }

    /// Loads this animation as a bevy [AnimationClip]
    ///
    /// [AnimationTargetId]s will be generated from the [Node::path].
    pub async fn load_animation_clip(&self, ctx: &mut LoadContext<'_>) -> Result<AnimationClip> {
        self.load_animation_clip_with_targets(ctx, |channel| {
            Some(bevy::animation::AnimationTargetId::from_names(
                channel.node().path().iter(),
            ))
        })
        .await
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

        // Check that the keyframes are valid
        let keyframes = sampler.input();
        if keyframes.is_sparse() {
            bevy::log::warn!("Sparse accessor not supported for animation sampler input");
            return Err(Error::UnsupportedAccessor);
        }
        let keyframes = keyframes.load::<f32>(ctx).await?;
        if keyframes.count() == 0 {
            bevy::log::warn!("Tried to load animation with no keyframe timestamps");
            return Err(Error::MissingKeyframeTimestamps);
        }

        let output = self.sampler().output().load_untyped(ctx).await?;

        macro_rules! make_curve {
            ($prop:expr,  $t:ty $(,$r:ident)?) => {{
                let values = output.try_with_type::<$t>()?;
                if keyframes.count() == 1 {
                    VariableCurve::new(AnimatableCurve::new(
                        $prop,
                        ConstantCurve::new(Interval::EVERYWHERE, values.get(0).unwrap()),
                    ))
                } else {
                    match self.sampler().interpolation() {
                        Interpolation::Linear => {

                            VariableCurve::new(AnimatableCurve::new(
                                $prop,
                                UnevenSampleAutoCurve::new(keyframes.iter().zip(values.iter()))
                                .map_err(|_| Error::InvalidAnimationCurve)?
                                ))
                        },
                        Interpolation::CubicSpline => VariableCurve::new(AnimatableCurve::new(
                            $prop,
                            make_curve!(@cubic $($r)? keyframes.iter(), values.iter())
                                .map_err(|_| Error::InvalidAnimationCurve)?,
                        )),
                        Interpolation::Step => VariableCurve::new(AnimatableCurve::new(
                            $prop,
                            SteppedKeyframeCurve::new(keyframes.iter().zip(values.iter())).map_err(|_| Error::InvalidAnimationCurve)?
                        )),
                    }
                }
            }};

            (@cubic rot $keyframes:expr, $values:expr) => {
                CubicRotationCurve::new($keyframes, $values.map(Vec4::from))
            };
            (@cubic $keyframes:expr, $values:expr) => {
                CubicKeyframeCurve::new($keyframes, $values)
            };
        }

        let curve = match self.property() {
            Property::Translation => make_curve!(animated_field!(Transform::translation), Vec3),
            Property::Rotation => make_curve!(animated_field!(Transform::rotation), Quat, rot),
            Property::Scale => make_curve!(animated_field!(Transform::scale), Vec3),
            _ => todo!("Morph target weights"),
        };

        Ok(curve)
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
