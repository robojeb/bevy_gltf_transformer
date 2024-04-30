//! Non-public utility structures and algorithms
use std::sync::RwLock;

use super::BufferId;
use bevy::utils::hashbrown::HashMap;

pub(crate) mod data_uri;
pub mod norm;

/// Cache for loaded glTF buffers
pub struct Cache {
    data: RwLock<HashMap<BufferId, OwningSlice>>,
}

impl Cache {
    pub fn empty() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }

    pub fn new(ptr: OwningSlice) -> Self {
        let mut map = HashMap::new();
        map.insert(BufferId::Bin, ptr);

        Self {
            data: RwLock::new(map),
        }
    }

    pub fn get(&self, id: BufferId) -> Option<&[u8]> {
        let read = self.data.read().unwrap();

        read.get(&id).map(|p| unsafe { p.slice() })
    }

    pub fn store(&self, id: BufferId, data: impl Into<Box<[u8]>>) -> &[u8] {
        let mut write = self.data.write().unwrap();
        write.insert(id, OwningSlice::new_complete(data.into()));
        unsafe { write.get(&id).unwrap().slice() }
    }
}

pub(crate) struct OwningSlice {
    /// The owning allocation
    allocation: Box<[u8]>,
    /// Start offset of the slice
    slice_offset: isize,
    /// Length of the slice
    slice_len: usize,
}

impl OwningSlice {
    pub(crate) fn find_offset(root: &[u8], slice: &[u8]) -> Option<isize> {
        let root_ptr = root.as_ptr();
        let slice_ptr = slice.as_ptr();

        unsafe {
            let offset = slice_ptr.offset_from(root_ptr);

            (offset >= 0 && ((offset + slice.len() as isize) < root.len() as isize))
                .then_some(offset)
        }
    }

    pub(crate) fn new_complete(allocation: Box<[u8]>) -> Self {
        let len = allocation.len();

        Self {
            allocation,
            slice_offset: 0,
            slice_len: len,
        }
    }

    pub(crate) unsafe fn new(allocation: Box<[u8]>, slice_offset: isize, slice_len: usize) -> Self {
        Self {
            allocation,
            slice_offset,
            slice_len,
        }
    }

    unsafe fn slice<'a>(&self) -> &'a [u8] {
        let slice = self.allocation.as_ptr().byte_offset(self.slice_offset);
        unsafe { std::slice::from_raw_parts(slice, self.slice_len) }
    }
}
