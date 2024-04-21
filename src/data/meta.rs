use gltf::accessor::sparse::IndexType;

use crate::wrap::ElementShape;

#[derive(Clone, Copy, Debug)]
pub struct Meta {
    pub(crate) shape: ElementShape,
    pub(crate) elem_size: usize,
    pub(crate) stride: usize,
    pub(crate) count: usize,
    pub(crate) normalized: bool,
}

impl Meta {
    pub fn from_accessor(acc: &gltf::Accessor<'_>) -> Self {
        Self {
            //extension: acc.extensions(),
            shape: ElementShape::from(acc),
            elem_size: acc.size(),
            count: acc.count(),
            stride: acc.view().and_then(|v| v.stride()).unwrap_or(acc.size()),
            normalized: acc.normalized(),
        }
    }

    pub fn from_sparse_index(acc: &gltf::Accessor<'_>) -> Self {
        let sparse = acc.sparse().unwrap();

        Self {
            //extension: None,
            shape: ElementShape::from(acc),
            elem_size: acc.size(),
            count: sparse.count(),
            stride: sparse.indices().view().stride().unwrap_or(
                match sparse.indices().index_type() {
                    IndexType::U8 => 1,
                    IndexType::U16 => 2,
                    IndexType::U32 => 4,
                },
            ),
            normalized: false,
        }
    }

    pub fn from_sparse_values(acc: &gltf::Accessor<'_>) -> Self {
        let sparse = acc.sparse().unwrap();

        Self {
            //extension: None,
            shape: ElementShape::from(acc),
            elem_size: acc.size(),
            count: sparse.count(),
            stride: sparse.values().view().stride().unwrap_or(acc.size()),
            normalized: acc.normalized(),
        }
    }
}
