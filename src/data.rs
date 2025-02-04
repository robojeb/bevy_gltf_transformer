//! Data streams from glTF Accessors
pub mod accessible;
pub mod dense;
mod meta;
pub mod sparse;

use crate::error::Result;
pub use accessible::{Accessible, AccessorData, Element};
pub use dense::{DenseData, DenseDataIter};
use gltf::accessor::{DataType, Dimensions};
pub(crate) use meta::Meta;
pub use sparse::{SparseData, SparseDataIter};

/// Static zero valued buffer for returning sparse untyped data
///
/// This should (hopefully) go into `.bss` so it shouldn't inflate the binary size
/// to prevent badly behaved accessors from messing us up we will over
/// allocate past the size of the largest supported accessor shape
static ZEROS: [u8; 256] = [0u8; 256];

macro_rules! each {
    ($s:ident.$call:ident ($($arg:expr),*)) => {
        match $s {
            Self::Dense(a) => a.$call($($arg,)*),
            Self::Sparse(a) => a.$call($($arg,)*),
        }
    };
}

/// A Data stream for elements from an [Accessor](crate::wrap::Accessor) converted
/// into an appropriate Rust type
pub enum Data<'a, T> {
    /// Densly packed Data
    Dense(DenseData<'a, T>),
    /// Sparse data relative to some base view, or zero
    Sparse(SparseData<'a, T>),
}

/// Loaded data from an accessor
impl<'a, T> Data<'a, T> {
    /// Get the raw bytes of an element from the accessor
    pub fn get_raw(&self, index: usize) -> Option<&'a [u8]> {
        each!(self.get_raw(index))
    }

    /// Try to convert the data stream to the provided type
    pub fn try_with_type<U>(&self) -> Result<Data<'a, U>>
    where
        U: Accessible,
    {
        match self {
            Self::Dense(d) => Ok(Data::Dense(d.try_with_type()?)),
            Self::Sparse(s) => Ok(Data::Sparse(s.try_with_type()?)),
        }
    }
}

impl<'a, T> Data<'a, T>
where
    T: Accessible,
{
    /// Try to get data from the accessor at the given index
    pub fn get(&self, index: usize) -> Option<T::Item> {
        match self {
            Self::Dense(d) => d.get(index),
            Self::Sparse(s) => s.get(index),
        }
    }

    /// Get the [Dimensions] of the data viewed by this accessor
    pub fn dimensions(&self) -> Dimensions {
        each!(self.dimensions())
    }

    /// Get the expected [DataType] of the elements in this accessor
    pub fn data_type(&self) -> DataType {
        each!(self.data_type())
    }

    /// The size in bytes of each element in this accessor
    pub fn element_size(&self) -> usize {
        each!(self.element_size())
    }

    /// The number of elements in this accessor
    ///
    /// *Note:* This is called `count` to mirror the name of the field in the
    /// GLTF metadata for the accessor.
    pub fn count(&self) -> usize {
        each!(self.count())
    }

    /// Specifies if the integer data values should be normalized
    ///
    /// This corresponds to using the `{U,S}norm` vertex attributes when
    /// constructing a mesh. For example if a normalized accessor has
    /// [DataType::U8] with [Dimensions::Vec2] then the vertex data should
    /// use [VertexFormat::Unorm8x2](bevy::render::render_resource::VertexFormat::Unorm8x2).
    pub fn normalized(&self) -> bool {
        each!(self.normalized())
    }

    /// Get an iterator over all the elements in the data stream
    pub fn iter(&self) -> DataIter<'a, T> {
        match self {
            Self::Dense(d) => DataIter::Dense(d.iter()),
            Self::Sparse(s) => DataIter::Sparse(s.iter()),
        }
    }
}

/// An iterator over elements in an accessor
pub enum DataIter<'a, T: Accessible> {
    /// Iterator over densly packed data
    Dense(DenseDataIter<'a, T>),
    /// Iterator over sparse data
    Sparse(SparseDataIter<'a, T>),
}

impl<T> Iterator for DataIter<'_, T>
where
    T: Accessible,
{
    type Item = T::Item;

    fn next(&mut self) -> Option<Self::Item> {
        each!(self.next())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }
}

impl<T> ExactSizeIterator for DataIter<'_, T>
where
    T: Accessible,
{
    fn len(&self) -> usize {
        each!(self.len())
    }
}

/// Marker type indicating no transformation is specified for the accessor
/// elements
pub struct Untyped;
