//! Structures for glTF accessors
//!
use super::{Document, View};
use crate::{
    data::{sparse::IndexData, Accessible, Data, DenseData, Meta, SparseData, Untyped},
    error::Result,
};
use bevy::asset::LoadContext;
use gltf::accessor::sparse::IndexType;
use serde_json::{value::RawValue, Value};

/// An accessor to data in some [View]
pub struct Accessor<'a> {
    doc: Document<'a>,
    raw: gltf::Accessor<'a>,
}

impl<'a> Accessor<'a> {
    pub(crate) fn new(doc: Document<'a>, raw: gltf::Accessor<'a>) -> Self {
        Self { doc, raw }
    }

    /// Load the data for this accessor without a specified transformation to
    /// rust types.
    pub async fn load_untyped(&self, ctx: &mut LoadContext<'_>) -> Result<Data<'a, Untyped>> {
        if let Some(sparse) = self.sparse() {
            let base = if let Some(base) = self.view() {
                let data = &base.load(ctx).await?[self.offset()..];

                Some(DenseData::new(Meta::from_accessor(&self.raw), data))
            } else {
                None
            };

            let indices = sparse.indices().load(ctx).await?;
            let values = sparse.values().load_untyped(ctx).await?;

            Ok(Data::Sparse(SparseData::new(
                Meta::from_accessor(&self.raw),
                base,
                indices,
                values,
            )))
        } else {
            let data = &self.view().unwrap().load(ctx).await?[self.offset()..];

            Ok(Data::Dense(DenseData::new(
                Meta::from_accessor(&self.raw),
                data,
            )))
        }
    }

    /// Load the data for this accessor with a transformation to the specified
    /// rust type `T`
    pub async fn load<T: Accessible>(&self, ctx: &mut LoadContext<'_>) -> Result<Data<T>> {
        self.load_untyped(ctx).await?.try_with_type()
    }

    /// Returns true if this accessor uses sparse data
    #[inline(always)]
    pub fn is_sparse(&self) -> bool {
        self.raw.sparse().is_some()
    }

    /// Returns information about sparse data storage if this accessor is sparse.
    #[inline(always)]
    pub fn sparse(&self) -> Option<Sparse<'a>> {
        self.raw.sparse().map(|s| Sparse {
            doc: self.doc,
            accessor: self.raw.clone(),
            raw: s,
        })
    }

    /// Returns the backing buffer view for this accessor
    ///
    /// This can be [None] for sparse accessors. All non-specified data is
    /// assumed to be zero when no backing view is provided.
    #[inline(always)]
    pub fn view(&self) -> Option<View<'a>> {
        self.raw.view().map(|v| View::new(self.doc, v))
    }

    /// Returns the offset into the parent buffer view in bytes.
    #[inline(always)]
    pub fn offset(&self) -> usize {
        self.raw.offset()
    }

    /// The number of elements in this accessor
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.raw.count()
    }

    /// Returns true if this accessor has no elements
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The size in bytes of a single element returned by this [Accessor]
    #[inline(always)]
    pub fn element_size(&self) -> usize {
        debug_assert_eq!(
            self.raw.size(),
            self.shape().size(),
            "Reported view size does not match size computed by reported type"
        );
        self.raw.size()
    }

    /// The shape of the data stored in each element
    pub fn shape(&self) -> ElementShape {
        let t = match self.raw.data_type() {
            gltf::accessor::DataType::F32 => ElementType::F32,
            gltf::accessor::DataType::I8 => ElementType::I8,
            gltf::accessor::DataType::U8 => ElementType::U8,
            gltf::accessor::DataType::I16 => ElementType::I16,
            gltf::accessor::DataType::U16 => ElementType::U16,
            gltf::accessor::DataType::U32 => ElementType::U32,
        };

        match self.raw.dimensions() {
            gltf::accessor::Dimensions::Scalar => ElementShape::Scalar(t),
            gltf::accessor::Dimensions::Vec2 => ElementShape::Vec2(t),
            gltf::accessor::Dimensions::Vec3 => ElementShape::Vec3(t),
            gltf::accessor::Dimensions::Vec4 => ElementShape::Vec4(t),
            gltf::accessor::Dimensions::Mat2 => ElementShape::Mat2(t),
            gltf::accessor::Dimensions::Mat3 => ElementShape::Mat3(t),
            gltf::accessor::Dimensions::Mat4 => ElementShape::Mat4(t),
        }
    }

    /// Returns the data type of components in the attribute.
    #[inline(always)]
    pub fn data_type(&self) -> gltf::accessor::DataType {
        self.raw.data_type()
    }

    /// Specifies if the attribute is a scalar, vector, or matrix.
    #[inline(always)]
    pub fn dimensions(&self) -> gltf::accessor::Dimensions {
        self.raw.dimensions()
    }

    /// Returns the minimum value of each component in this attribute.
    #[inline(always)]
    pub fn min(&self) -> Option<Value> {
        self.raw.min()
    }

    /// Returns the maximum value of each component in this attribute.
    #[inline(always)]
    pub fn max(&self) -> Option<Value> {
        self.raw.max()
    }

    /// Specifies whether integer data values should be normalized.
    #[inline(always)]
    pub fn normalized(&self) -> bool {
        self.raw.normalized()
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
}

/// The dimensions and type of data from an [Accessor]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ElementShape {
    /// Individual element types
    Scalar(ElementType),
    /// A 2d vector of the specified element type
    Vec2(ElementType),
    /// A 3d vector of the specified element type
    Vec3(ElementType),
    /// A 4d vector of the specified element type
    Vec4(ElementType),
    /// A 2x2 column major matrix of the specified element type
    Mat2(ElementType),
    /// A 3x3 column major matrix of the specified element type
    Mat3(ElementType),
    /// A 4x4 column major matrix of the specified element type
    Mat4(ElementType),
}

impl<'a> From<&gltf::Accessor<'a>> for ElementShape {
    fn from(value: &gltf::Accessor<'a>) -> Self {
        let t = match value.data_type() {
            gltf::accessor::DataType::F32 => ElementType::F32,
            gltf::accessor::DataType::I8 => ElementType::I8,
            gltf::accessor::DataType::U8 => ElementType::U8,
            gltf::accessor::DataType::I16 => ElementType::I16,
            gltf::accessor::DataType::U16 => ElementType::U16,
            gltf::accessor::DataType::U32 => ElementType::U32,
        };

        match &value.dimensions() {
            gltf::accessor::Dimensions::Scalar => ElementShape::Scalar(t),
            gltf::accessor::Dimensions::Vec2 => ElementShape::Vec2(t),
            gltf::accessor::Dimensions::Vec3 => ElementShape::Vec3(t),
            gltf::accessor::Dimensions::Vec4 => ElementShape::Vec4(t),
            gltf::accessor::Dimensions::Mat2 => ElementShape::Mat2(t),
            gltf::accessor::Dimensions::Mat3 => ElementShape::Mat3(t),
            gltf::accessor::Dimensions::Mat4 => ElementShape::Mat4(t),
        }
    }
}

impl ElementShape {
    /// The expected size of this shape in bytes
    pub fn size(&self) -> usize {
        match self {
            ElementShape::Scalar(t) => t.size(),
            ElementShape::Vec2(t) => 2 * t.size(),
            ElementShape::Vec3(t) => 3 * t.size(),
            ElementShape::Vec4(t) => 4 * t.size(),
            ElementShape::Mat2(t) => 4 * t.size(),
            ElementShape::Mat3(t) => 9 * t.size(),
            ElementShape::Mat4(t) => 16 * t.size(),
        }
    }

    /// Get the [DataType](gltf::accessor::DataType) for this shape
    pub fn data_type(&self) -> gltf::accessor::DataType {
        match self {
            ElementShape::Mat2(t)
            | ElementShape::Scalar(t)
            | ElementShape::Vec2(t)
            | ElementShape::Vec3(t)
            | ElementShape::Vec4(t)
            | ElementShape::Mat3(t)
            | ElementShape::Mat4(t) => match t {
                ElementType::U8 => gltf::accessor::DataType::U8,
                ElementType::I8 => gltf::accessor::DataType::I8,
                ElementType::U16 => gltf::accessor::DataType::U16,
                ElementType::I16 => gltf::accessor::DataType::I16,
                ElementType::U32 => gltf::accessor::DataType::U32,
                ElementType::F32 => gltf::accessor::DataType::F32,
            },
        }
    }

    /// Get the [Dimensions](gltf::accessor::Dimensions) for this shape
    pub fn dimensions(&self) -> gltf::accessor::Dimensions {
        match self {
            ElementShape::Scalar(_) => gltf::accessor::Dimensions::Scalar,
            ElementShape::Vec2(_) => gltf::accessor::Dimensions::Vec2,
            ElementShape::Vec3(_) => gltf::accessor::Dimensions::Vec3,
            ElementShape::Vec4(_) => gltf::accessor::Dimensions::Vec4,
            ElementShape::Mat2(_) => gltf::accessor::Dimensions::Mat2,
            ElementShape::Mat3(_) => gltf::accessor::Dimensions::Mat3,
            ElementShape::Mat4(_) => gltf::accessor::Dimensions::Mat4,
        }
    }
}

/// Individual element type for an [Accessor]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ElementType {
    /// [u8] element type
    U8,
    /// [i8] element type
    I8,
    /// [u16] element type
    U16,
    /// [i16] element type
    I16,
    /// [u32] element type
    U32,
    /// [f32] element type
    F32,
}

impl ElementType {
    /// Size of a single element in bytes
    pub fn size(&self) -> usize {
        match self {
            ElementType::U8 => 1,
            ElementType::I8 => 1,
            ElementType::U16 => 2,
            ElementType::I16 => 2,
            ElementType::U32 => 4,
            ElementType::F32 => 4,
        }
    }
}

/// Information about sparse data storage for an [Accessor]
pub struct Sparse<'a> {
    doc: Document<'a>,
    accessor: gltf::Accessor<'a>,
    raw: gltf::accessor::sparse::Sparse<'a>,
}

impl<'a> Sparse<'a> {
    /// Returns the number of attributes encoded in this sparse accessor.
    #[inline(always)]
    pub fn count(&self) -> usize {
        self.raw.count()
    }

    /// Data about the element indices which have replacement values
    #[inline(always)]
    pub fn indices(&self) -> Indices<'a> {
        Indices {
            doc: self.doc,
            accessor: self.accessor.clone(),
            raw: self.raw.indices(),
        }
    }

    /// Replacement values for sparse storage
    #[inline(always)]
    pub fn values(&self) -> Values<'a> {
        Values {
            doc: self.doc,
            accessor: self.accessor.clone(),
            raw: self.raw.values(),
        }
    }
}

/// Information to access the sparse data indices
pub struct Indices<'a> {
    accessor: gltf::Accessor<'a>,
    raw: gltf::accessor::sparse::Indices<'a>,
    doc: Document<'a>,
}

impl<'a> Indices<'a> {
    /// Returns the buffer view containing the sparse indices.
    #[inline(always)]
    pub fn view(&self) -> View<'a> {
        View::new(self.doc, self.raw.view())
    }

    /// The offset relative to the start of the parent buffer view in bytes.
    #[inline(always)]
    pub fn offset(&self) -> usize {
        self.raw.offset()
    }

    /// The data type of each index.
    #[inline(always)]
    pub fn index_type(&self) -> IndexType {
        self.raw.index_type()
    }

    /// Load the data for the sparse indices as the appropriate index type
    pub async fn load(&self, ctx: &mut LoadContext<'_>) -> Result<IndexData<'a>> {
        let view = &self.view().load(ctx).await?[self.offset()..];

        let untyped = DenseData::<'a, Untyped>::new(Meta::from_sparse_index(&self.accessor), view);

        Ok(match self.index_type() {
            IndexType::U8 => IndexData::U8(untyped.try_with_type()?),
            IndexType::U16 => IndexData::U16(untyped.try_with_type()?),
            IndexType::U32 => IndexData::U32(untyped.try_with_type()?),
        })
    }
}

/// Information about the replacement values for a sparse [Accessor]
///
/// The [ElementShape] of these values should match the base [Accessor] if it
/// has a specified base [View].
pub struct Values<'a> {
    accessor: gltf::Accessor<'a>,
    raw: gltf::accessor::sparse::Values<'a>,
    doc: Document<'a>,
}

impl<'a> Values<'a> {
    /// Returns the buffer view containing the sparse indices.
    #[inline(always)]
    pub fn view(&self) -> View<'a> {
        View::new(self.doc, self.raw.view())
    }

    /// The offset relative to the start of the parent buffer view in bytes.
    #[inline(always)]
    pub fn offset(&self) -> usize {
        self.raw.offset()
    }

    /// Load the data for the sparse values without a transformation to an
    /// rust type.
    pub async fn load_untyped(&self, ctx: &mut LoadContext<'_>) -> Result<DenseData<'a, Untyped>> {
        let view = &self.view().load(ctx).await?[self.offset()..];

        Ok(DenseData::new(
            Meta::from_sparse_values(&self.accessor),
            view,
        ))
    }
}
