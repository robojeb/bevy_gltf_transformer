//! Types and traits for conversion from glTF accessor types to rust types
use crate::wrap::{ElementShape, ElementType};
use bevy::math::{Mat2, Mat3, Mat3A, Mat4, Quat, Vec2, Vec3, Vec3A, Vec4};
use gltf::accessor::{DataType, Dimensions};

/// A raw element from an accessor with its byte data and associated expected
/// shape
#[derive(Clone, Copy)]
pub struct Element<'a> {
    /// The raw element bytes
    pub data: &'a [u8],
    /// The expected data shape
    pub shape: ElementShape,
}

impl<'a> Element<'a> {
    /// Consume a [u8] from the [Element]
    pub fn read_u8(&mut self) -> u8 {
        let out = self.data[0];
        self.data = &self.data[1..];
        out
    }

    /// Consume an [i8] from the [Element]
    pub fn read_i8(&mut self) -> i8 {
        self.read_u8() as i8
    }

    /// Consume a [u16] from the [Element]
    pub fn read_u16(&mut self) -> u16 {
        let Some((out, data)) = self.data.split_first_chunk() else {
            panic!("Not enough bytes to read u16")
        };
        self.data = data;
        u16::from_le_bytes(*out)
    }

    /// Consume an [i16] from the [Element]
    pub fn read_i16(&mut self) -> i16 {
        self.read_u16() as i16
    }

    /// Consume a [u32] from the [Element]
    pub fn read_u32(&mut self) -> u32 {
        let Some((out, data)) = self.data.split_first_chunk() else {
            panic!("Not enough bytes to read u32")
        };
        self.data = data;
        u32::from_le_bytes(*out)
    }

    /// Consume an [f32] from the [Element]
    pub fn read_f32(&mut self) -> f32 {
        let Some((out, data)) = self.data.split_first_chunk() else {
            panic!("Not enough bytes to read f32")
        };
        self.data = data;
        f32::from_le_bytes(*out)
    }
}

/// A trait for types which can convert glTF accessor elements into rust types.
///
/// The rust type's does not have to match the element type or dimensionality,
/// and may be able to convert from multiple glTF element shapes into the same
/// rust type.
pub trait Accessible {
    /// The target rust element type
    ///
    /// This can be `Self` or some other type for semantic accessors like
    /// `SrgbaColor`.
    type Item;

    /// The "zero" value of the elment
    ///
    /// This is used for sparse accessors that do not have a defined base view.
    /// Any element which does not have a specified sparse value will get
    /// [Self::zero()].
    fn zero(shape: ElementShape) -> Self::Item;

    /// Convert the provided element into the destination rust type
    fn from_element(elem: Element) -> Self::Item;

    /// Confirm that given the accessor's [ElementShape] this type can
    /// successfully produce the target rust type
    fn validate_accessor(shape: ElementShape) -> bool;
}

impl<T> Accessible for T
where
    T: AccessorShape,
{
    type Item = Self;

    fn zero(_shape: ElementShape) -> Self {
        Self::ZERO
    }

    fn from_element(elem: Element) -> Self {
        <T as AccessorShape>::from_element(elem)
    }

    fn validate_accessor(shape: ElementShape) -> bool {
        shape.data_type() == <T::Data as AccessorData>::KIND && shape.dimensions() == T::DIM
    }
}

/// A helper trait for mapping rust types to glTF data-types
pub trait AccessorData: Copy {
    /// The glTF [DataType] associated with this rust type
    const KIND: DataType;
    /// The zero value for this type
    const ZERO: Self;

    /// Get the data from a byte buffer and advance the buffer
    fn get(elem: &mut Element) -> Self;
}

/// A helper trait for mapping glTF accessor element dimensions to rust
/// arrays
pub trait AccessorShape {
    /// The underlying data type of each element
    type Data: AccessorData;
    /// The dimensionality of the result
    const DIM: Dimensions;
    /// The zero value of the element
    const ZERO: Self;

    /// Convert the provided element into the destination shape
    fn from_element(elem: Element) -> Self;
}

/// Implement scalar shape for all types
impl<T> AccessorShape for T
where
    T: AccessorData,
{
    type Data = Self;
    const DIM: Dimensions = Dimensions::Scalar;
    const ZERO: Self = <Self as AccessorData>::ZERO;

    #[inline(always)]
    fn from_element(mut elem: Element) -> Self {
        T::get(&mut elem)
    }
}

impl AccessorData for u8 {
    const KIND: DataType = DataType::U8;
    const ZERO: Self = 0;

    fn get(elem: &mut Element) -> Self {
        elem.read_u8()
    }
}

impl AccessorData for i8 {
    const KIND: DataType = DataType::I8;
    const ZERO: Self = 0;

    fn get(elem: &mut Element) -> Self {
        elem.read_i8()
    }
}

macro_rules! impl_accessor_data {
    ($t:ty, $kind:expr, $get:ident) => {
        impl AccessorData for $t {
            const KIND: DataType = $kind;
            const ZERO: Self = 0;

            fn get(elem: &mut Element) -> Self {
                elem.$get()
            }
        }
    };
}

impl_accessor_data!(u16, DataType::U16, read_u16);
impl_accessor_data!(i16, DataType::I16, read_i16);
impl_accessor_data!(u32, DataType::U32, read_u32);

impl AccessorData for f32 {
    const KIND: DataType = DataType::F32;
    const ZERO: Self = 0.0;

    fn get(elem: &mut Element) -> Self {
        elem.read_f32()
    }
}

impl<T: AccessorData> AccessorShape for [T; 2] {
    type Data = T;
    const DIM: Dimensions = Dimensions::Vec2;
    const ZERO: Self = [T::ZERO; 2];

    fn from_element(mut elem: Element) -> Self {
        let data = &mut elem;
        [T::get(data), T::get(data)]
    }
}

impl<T: AccessorData> AccessorShape for [T; 3] {
    type Data = T;
    const DIM: Dimensions = Dimensions::Vec3;
    const ZERO: Self = [T::ZERO; 3];

    fn from_element(mut elem: Element) -> Self {
        let data = &mut elem;
        [T::get(data), T::get(data), T::get(data)]
    }
}

impl<T: AccessorData> AccessorShape for [T; 4] {
    type Data = T;
    const DIM: Dimensions = Dimensions::Vec4;
    const ZERO: Self = [T::ZERO; 4];

    fn from_element(mut elem: Element) -> Self {
        let data = &mut elem;
        [T::get(data), T::get(data), T::get(data), T::get(data)]
    }
}

impl<T: AccessorData> AccessorShape for [[T; 2]; 2] {
    type Data = T;
    const DIM: Dimensions = Dimensions::Mat2;
    const ZERO: Self = [[T::ZERO; 2]; 2];

    fn from_element(mut elem: Element) -> Self {
        let data = &mut elem;
        [[T::get(data), T::get(data)], [T::get(data), T::get(data)]]
    }
}

impl<T: AccessorData> AccessorShape for [[T; 3]; 3] {
    type Data = T;
    const DIM: Dimensions = Dimensions::Mat3;
    const ZERO: Self = [[T::ZERO; 3]; 3];

    fn from_element(mut elem: Element) -> Self {
        let data = &mut elem;
        [
            [T::get(data), T::get(data), T::get(data)],
            [T::get(data), T::get(data), T::get(data)],
            [T::get(data), T::get(data), T::get(data)],
        ]
    }
}

impl<T: AccessorData> AccessorShape for [[T; 4]; 4] {
    type Data = T;
    const DIM: Dimensions = Dimensions::Mat4;
    const ZERO: Self = [[T::ZERO; 4]; 4];

    fn from_element(mut elem: Element) -> Self {
        let data = &mut elem;
        [
            [T::get(data), T::get(data), T::get(data), T::get(data)],
            [T::get(data), T::get(data), T::get(data), T::get(data)],
            [T::get(data), T::get(data), T::get(data), T::get(data)],
            [T::get(data), T::get(data), T::get(data), T::get(data)],
        ]
    }
}

impl Accessible for Vec2 {
    type Item = Vec2;

    fn from_element(mut elem: Element) -> Self::Item {
        Vec2 {
            x: elem.read_f32(),
            y: elem.read_f32(),
        }
    }

    fn zero(_shape: ElementShape) -> Self::Item {
        Vec2::ZERO
    }

    fn validate_accessor(shape: ElementShape) -> bool {
        matches!(shape, ElementShape::Vec2(ElementType::F32))
    }
}

impl Accessible for Vec3 {
    type Item = Vec3;

    fn from_element(mut elem: Element) -> Self::Item {
        Vec3 {
            x: elem.read_f32(),
            y: elem.read_f32(),
            z: elem.read_f32(),
        }
    }

    fn zero(_shape: ElementShape) -> Self::Item {
        Vec3::ZERO
    }

    fn validate_accessor(shape: ElementShape) -> bool {
        matches!(shape, ElementShape::Vec3(ElementType::F32))
    }
}

impl Accessible for Vec3A {
    type Item = Vec3A;

    fn from_element(mut elem: Element) -> Self::Item {
        Vec3A::new(elem.read_f32(), elem.read_f32(), elem.read_f32())
    }

    fn zero(_shape: ElementShape) -> Self::Item {
        Vec3A::ZERO
    }

    fn validate_accessor(shape: ElementShape) -> bool {
        matches!(shape, ElementShape::Vec3(ElementType::F32))
    }
}

impl Accessible for Quat {
    type Item = Quat;

    fn from_element(mut elem: Element) -> Self::Item {
        Quat::from_xyzw(
            elem.read_f32(),
            elem.read_f32(),
            elem.read_f32(),
            elem.read_f32(),
        )
    }

    fn zero(_shape: ElementShape) -> Self::Item {
        Quat::IDENTITY
    }

    fn validate_accessor(shape: ElementShape) -> bool {
        matches!(shape, ElementShape::Vec4(ElementType::F32))
    }
}

impl Accessible for Mat2 {
    type Item = Mat2;

    fn from_element(mut elem: Element) -> Self::Item {
        Mat2::from_cols(
            Vec2 {
                x: elem.read_f32(),
                y: elem.read_f32(),
            },
            Vec2 {
                x: elem.read_f32(),
                y: elem.read_f32(),
            },
        )
    }

    fn zero(_shape: ElementShape) -> Self::Item {
        Mat2::ZERO
    }

    fn validate_accessor(shape: ElementShape) -> bool {
        matches!(shape, ElementShape::Mat2(ElementType::F32))
    }
}

impl Accessible for Mat3 {
    type Item = Mat3;

    fn from_element(mut elem: Element) -> Self::Item {
        Mat3::from_cols(
            Vec3 {
                x: elem.read_f32(),
                y: elem.read_f32(),
                z: elem.read_f32(),
            },
            Vec3 {
                x: elem.read_f32(),
                y: elem.read_f32(),
                z: elem.read_f32(),
            },
            Vec3 {
                x: elem.read_f32(),
                y: elem.read_f32(),
                z: elem.read_f32(),
            },
        )
    }

    fn zero(_shape: ElementShape) -> Self::Item {
        Mat3::ZERO
    }

    fn validate_accessor(shape: ElementShape) -> bool {
        matches!(shape, ElementShape::Mat3(ElementType::F32))
    }
}

impl Accessible for Mat3A {
    type Item = Mat3A;

    fn from_element(mut elem: Element) -> Self::Item {
        Mat3A::from_cols(
            Vec3A::new(elem.read_f32(), elem.read_f32(), elem.read_f32()),
            Vec3A::new(elem.read_f32(), elem.read_f32(), elem.read_f32()),
            Vec3A::new(elem.read_f32(), elem.read_f32(), elem.read_f32()),
        )
    }

    fn zero(_shape: ElementShape) -> Self::Item {
        Mat3A::ZERO
    }

    fn validate_accessor(shape: ElementShape) -> bool {
        matches!(shape, ElementShape::Mat3(ElementType::F32))
    }
}

impl Accessible for Mat4 {
    type Item = Mat4;

    fn from_element(mut elem: Element) -> Self::Item {
        Mat4::from_cols(
            Vec4::new(
                elem.read_f32(),
                elem.read_f32(),
                elem.read_f32(),
                elem.read_f32(),
            ),
            Vec4::new(
                elem.read_f32(),
                elem.read_f32(),
                elem.read_f32(),
                elem.read_f32(),
            ),
            Vec4::new(
                elem.read_f32(),
                elem.read_f32(),
                elem.read_f32(),
                elem.read_f32(),
            ),
            Vec4::new(
                elem.read_f32(),
                elem.read_f32(),
                elem.read_f32(),
                elem.read_f32(),
            ),
        )
    }

    fn zero(_shape: ElementShape) -> Self::Item {
        Mat4::ZERO
    }

    fn validate_accessor(shape: ElementShape) -> bool {
        matches!(shape, ElementShape::Mat4(ElementType::F32))
    }
}
