//! Types for accessing dense accessor data
use super::{
    accessible::{Accessible, Element},
    meta::Meta,
    Untyped,
};
use crate::error::{Error, Result};

use gltf::accessor::{DataType, Dimensions};
use std::marker::PhantomData;

/// Dense accessor data
pub struct DenseData<'a, T> {
    /// Accessor meta-data
    pub(crate) meta: Meta,
    /// Buffer data-view
    view: &'a [u8],
    /// Type info
    _element: PhantomData<T>,
}

impl<'a, T> Clone for DenseData<'a, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T> Copy for DenseData<'a, T> {}

impl<'a, T> DenseData<'a, T> {
    /// Create a new [DenseData] from a view and provided meta-data
    pub(crate) fn new(meta: Meta, view: &'a [u8]) -> Self {
        Self {
            meta,
            view,
            _element: PhantomData,
        }
    }

    /// Access the raw data for the element at the specified index
    pub fn get_raw(&self, index: usize) -> Option<&'a [u8]> {
        let stride = self.meta.stride;

        let raw_index = index.checked_mul(stride)?;
        let raw_end_index = raw_index.checked_add(self.element_size())?;

        (self.count() > index && raw_index < self.view.len() && raw_end_index < self.view.len())
            .then(|| {
                // Extract the requested data
                &self.view[raw_index..raw_end_index]
            })
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
    pub fn try_with_type<U>(&self) -> Result<DenseData<'a, U>>
    where
        U: Accessible,
    {
        U::validate_accessor(self.meta.shape)
            .then_some(DenseData {
                meta: self.meta,
                view: self.view,
                _element: PhantomData,
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
    /// requested `U` can be constructed.
    pub unsafe fn with_type<U>(self) -> DenseData<'a, U> {
        DenseData {
            meta: self.meta,
            view: self.view,
            _element: PhantomData,
        }
    }
}

impl<'a, T> DenseData<'a, T>
where
    T: Accessible,
{
    /// Get an element from this accessor and interpret as rust data
    pub fn get(&self, index: usize) -> Option<T::Item> {
        self.get_raw(index).map(|data| {
            T::from_element(Element {
                data,
                shape: self.meta.shape,
            })
        })
    }

    /// Iterate over all the elements in this accessor
    pub fn iter(&self) -> DenseDataIter<'a, T> {
        DenseDataIter::new(self)
    }

    /// Get an untyped view of this data
    pub fn as_untyped(&self) -> DenseData<'a, Untyped> {
        DenseData {
            meta: self.meta,
            view: self.view,
            _element: PhantomData,
        }
    }
}

/// Iterator over densly packed accessor data
pub struct DenseDataIter<'a, T> {
    counter: usize,
    pub(crate) accessor: DenseData<'a, T>,
}

impl<'a, T> DenseDataIter<'a, T> {
    /// Create a new iterator from [DenseData]
    pub fn new(accessor: &DenseData<'a, T>) -> Self {
        Self {
            counter: 0,
            accessor: *accessor,
        }
    }
}

impl<'a, T> Iterator for DenseDataIter<'a, T>
where
    T: Accessible,
{
    type Item = T::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if self.counter < self.accessor.count() {
            let out = self.accessor.get(self.counter)?;
            self.counter += 1;
            Some(out)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }
}

impl<'a, T> ExactSizeIterator for DenseDataIter<'a, T>
where
    T: Accessible,
{
    fn len(&self) -> usize {
        self.accessor.meta.count - self.counter
    }
}
