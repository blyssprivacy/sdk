use std::{
    alloc::{alloc, alloc_zeroed, dealloc, realloc, Layout},
    mem::size_of,
    ops::{Index, IndexMut},
    slice::{from_raw_parts, from_raw_parts_mut},
};

const ALIGN_SIMD: usize = 64; // enough to support AVX-512
pub type AlignedMemory64 = AlignedMemory<ALIGN_SIMD>;

pub struct AlignedMemory<const ALIGN: usize> {
    p: *mut u64,
    sz_u64: usize,
    layout: Layout,
}

impl<const ALIGN: usize> AlignedMemory<{ ALIGN }> {
    fn _new(sz_u64: usize, zeroed: bool) -> Self {
        let sz_bytes = sz_u64 * size_of::<u64>();
        let layout = Layout::from_size_align(sz_bytes, ALIGN).unwrap();

        let ptr;
        unsafe {
            ptr = if zeroed {
                alloc_zeroed(layout)
            } else {
                alloc(layout)
            };
        }

        Self {
            p: ptr as *mut u64,
            sz_u64,
            layout,
        }
    }

    pub fn new(sz_u64: usize) -> Self {
        Self::_new(sz_u64, true)
    }

    pub fn new_empty(sz_u64: usize) -> Self {
        Self::_new(sz_u64, false)
    }

    pub fn extend(&mut self, new_size: usize) {
        self.sz_u64 = new_size;
        let new_size = self.sz_u64 * size_of::<u64>();
        unsafe {
            self.p = realloc(self.p as *mut u8, self.layout, new_size) as *mut u64;
            self.layout = Layout::from_size_align_unchecked(new_size, self.layout.align());
        }
    }

    pub fn insert(&mut self, index: usize, element: u64) {
        self.extend(self.sz_u64 + 1);
        self.as_mut_slice()[index..].rotate_right(1);
        self[index] = element;
    }

    pub fn as_slice(&self) -> &[u64] {
        unsafe { from_raw_parts(self.p, self.sz_u64) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [u64] {
        unsafe { from_raw_parts_mut(self.p, self.sz_u64) }
    }

    pub unsafe fn as_ptr(&self) -> *const u64 {
        self.p
    }

    pub unsafe fn as_mut_ptr(&mut self) -> *mut u64 {
        self.p
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe { from_raw_parts(self.p as *const u8, self.sz_u64 * size_of::<u64>()) }
    }

    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        unsafe { from_raw_parts_mut(self.p as *mut u8, self.sz_u64 * size_of::<u64>()) }
    }

    pub fn len(&self) -> usize {
        self.sz_u64
    }
}

unsafe impl<const ALIGN: usize> Send for AlignedMemory<{ ALIGN }> {}
unsafe impl<const ALIGN: usize> Sync for AlignedMemory<{ ALIGN }> {}

impl<const ALIGN: usize> Drop for AlignedMemory<{ ALIGN }> {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.p as *mut u8, self.layout);
        }
    }
}

impl<const ALIGN: usize> Index<usize> for AlignedMemory<{ ALIGN }> {
    type Output = u64;

    fn index(&self, index: usize) -> &Self::Output {
        &self.as_slice()[index]
    }
}

impl<const ALIGN: usize> IndexMut<usize> for AlignedMemory<{ ALIGN }> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.as_mut_slice()[index]
    }
}

impl<const ALIGN: usize> Clone for AlignedMemory<{ ALIGN }> {
    fn clone(&self) -> Self {
        let mut out = Self::new(self.sz_u64);
        out.as_mut_slice().copy_from_slice(self.as_slice());
        out
    }
}
