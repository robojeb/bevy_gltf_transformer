//! Structures for handling sparse glTF accessor data
//!
use std::iter::{Peekable, Zip};

use super::{dense::DenseDataIter, Accessible, DenseData, Meta};
use crate::error::{Error, Result};
use gltf::accessor::{DataType, Dimensions};

/// A structure to access sparse accessor data
pub struct SparseData<'a, T> {
    meta: Meta,
    base: Option<DenseData<'a, T>>,
    indices: IndexData<'a>,
    values: DenseData<'a, T>,
}

impl<'a, T> SparseData<'a, T> {
    pub(crate) fn new(
        meta: Meta,
        base: Option<DenseData<'a, T>>,
        indices: IndexData<'a>,
        values: DenseData<'a, T>,
    ) -> Self {
        Self {
            meta,
            base,
            indices,
            values,
        }
    }

    /// Get the raw bytes of an element from the accessor
    pub fn get_raw(&self, index: usize) -> Option<&'a [u8]> {
        match self.indices.find_replacement(index) {
            Some(replace_idx) => self.values.get_raw(replace_idx),
            None => self
                .base
                .as_ref()
                .map(|d| d.get_raw(index))
                .unwrap_or(Some(&super::ZEROS[..self.meta.elem_size])),
        }
    }

    /// Get the [Dimensions] of the data viewed by this accessor
    pub fn dimensions(&self) -> Dimensions {
        self.meta.shape.dimensions()
    }

    /// Get the expected [DataType] of the elements in this accessor
    pub fn data_type(&self) -> DataType {
        self.meta.shape.data_type()
    }

    /// The size in bytes of each element in this accessor
    pub fn element_size(&self) -> usize {
        self.meta.elem_size
    }

    /// The number of elements in this accessor
    pub fn count(&self) -> usize {
        self.meta.count
    }

    /// Specifies if the integer data values should be normalized
    ///
    /// This corresponds to using the `{U,S}norm` vertex attributes when
    /// constructing a mesh. For example if a normalized accessor has
    /// [DataType::U8] with [Dimensions::Vec2] then the vertex data should
    /// use [VertexFormat::Unorm8x2](bevy::render::render_resource::VertexFormat::Unorm8x2).
    pub fn normalized(&self) -> bool {
        self.meta.normalized
    }

    /// Attempt to convert this untyped accessor into a typed accessor.
    ///
    /// This will fail if the [DataType] and [Dimensions] of the requested
    /// type `T` do not match that of this accessor.
    pub fn try_with_type<U>(&self) -> Result<SparseData<'a, U>>
    where
        U: Accessible,
    {
        U::validate_accessor(self.meta.shape)
            .then(|| unsafe {
                SparseData {
                    base: self.base.map(|b| b.with_type()),
                    indices: self.indices,
                    values: self.values.with_type(),
                    meta: self.meta,
                }
            })
            .ok_or(Error::AccessorType {
                requested: std::any::type_name::<U>(),
                dt: self.data_type(),
                dim: self.dimensions(),
            })
    }

    /// Convert this to a typed accessor without vaidating that the accessor's
    /// specified data-type or dimensions match.
    ///
    /// # Safety
    /// The user must ensure that regardless of the size of each element the
    /// requested `T` can be constructed.
    pub unsafe fn with_type<U>(self) -> SparseData<'a, U> {
        SparseData {
            base: self.base.map(|b| b.with_type()),
            indices: self.indices,
            values: self.values.with_type(),
            meta: self.meta,
        }
    }
}

impl<'a, T> SparseData<'a, T>
where
    T: Accessible,
{
    ///  Get an element from this accessor interpreted a s rust data
    pub fn get(&self, index: usize) -> Option<T::Item> {
        match self.indices.find_replacement(index) {
            Some(replace_idx) => self.values.get(replace_idx),
            None => self
                .base
                .as_ref()
                .map(|d| d.get(index))
                .unwrap_or(Some(T::zero(self.meta.shape))),
        }
    }

    /// Get an iterator over the elements of a [SparseData] structure
    pub fn iter(&self) -> SparseDataIter<'a, T> {
        SparseDataIter {
            counter: 0,
            meta: self.meta,
            replace: self.indices.iter().zip(self.values.iter()).peekable(),
            base: self.base.as_ref().map(|b| b.iter()),
        }
    }
}

/// An iterator over sparse accessor data
pub struct SparseDataIter<'a, T: Accessible> {
    counter: usize,
    meta: Meta,
    replace: Peekable<Zip<IndexIter<'a>, DenseDataIter<'a, T>>>,
    base: Option<DenseDataIter<'a, T>>,
}

impl<'a, T> Iterator for SparseDataIter<'a, T>
where
    T: Accessible,
{
    type Item = T::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if self.counter >= self.meta.count {
            return None;
        }

        match self.replace.peek() {
            Some((idx, _)) if *idx == self.counter => {
                self.counter += 1;
                Some(self.replace.next().unwrap().1)
            }
            _ => {
                if let Some(ref mut base) = self.base {
                    base.next()
                } else {
                    Some(T::zero(self.meta.shape))
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }
}

impl<'a, T> ExactSizeIterator for SparseDataIter<'a, T>
where
    T: Accessible,
{
    fn len(&self) -> usize {
        self.meta.count - self.counter
    }
}

/// A structure containing index information for elements that are modified in
/// a sparse accessor
#[derive(Clone, Copy)]
pub enum IndexData<'a> {
    /// [u8] sized indices
    U8(DenseData<'a, u8>),
    /// [u16] sized indices
    U16(DenseData<'a, u16>),
    /// [u32] sized indices
    U32(DenseData<'a, u32>),
}

impl<'a> IndexData<'a> {
    /// The number of element indices
    pub fn count(&self) -> usize {
        match self {
            Self::U8(d) => d.count(),
            Self::U16(d) => d.count(),
            Self::U32(d) => d.count(),
        }
    }

    /// Returns the index in the sparse `values` array which corresponds to the
    /// accessor `index` if it exists
    pub fn find_replacement(&self, index: usize) -> Option<usize> {
        macro_rules! bin_search {
            ($d:expr) => {{
                let mut left = 0;
                let mut right = $d.count() - 1;

                while left != right {
                    let idx = (left + right).div_ceil(2);

                    let replaces = $d.get(idx)?;

                    match (replaces as usize).cmp(&index) {
                        std::cmp::Ordering::Equal => return Some(idx),
                        std::cmp::Ordering::Less => left = idx + 1,
                        std::cmp::Ordering::Greater => right = idx,
                    }
                }

                // Check the final index
                $d.get(left)
                    .and_then(|r| (r as usize == index).then_some(left))
            }};
        }

        match self {
            Self::U8(d) => bin_search!(d),
            Self::U16(d) => bin_search!(d),
            Self::U32(d) => bin_search!(d),
        }
    }

    /// Get the index of the n-th element to be replaced
    pub fn get(&self, n: usize) -> Option<usize> {
        match self {
            Self::U8(d) => d.get(n).map(|v| v as usize),
            Self::U16(d) => d.get(n).map(|v| v as usize),
            Self::U32(d) => d.get(n).map(|v| v as usize),
        }
    }

    fn iter(&self) -> IndexIter<'a> {
        IndexIter {
            counter: 0,
            indices: *self,
        }
    }
}

struct IndexIter<'a> {
    counter: usize,
    indices: IndexData<'a>,
}

impl<'a> Iterator for IndexIter<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.counter < self.indices.count() {
            let out = self.indices.get(self.counter);
            self.counter += 1;
            out
        } else {
            None
        }
    }
}
