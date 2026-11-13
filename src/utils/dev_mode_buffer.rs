use std::{
    alloc::{alloc_zeroed, dealloc, handle_alloc_error, Layout},
    mem::{align_of, size_of},
    ops::{Deref, DerefMut},
    ptr::NonNull,
    slice,
};
use windows::Win32::Graphics::Gdi::DEVMODEW;

/// A zero-initialized byte buffer that is aligned for `DEVMODEW` and holds at
/// least one whole `DEVMODEW`.
///
/// Windows APIs read and write `DEVMODEW` members through the pointer, so a
/// `Vec<u8>`, which has no alignment for `DEVMODEW`, is not sufficient.
pub(crate) struct DevModeBuffer {
    ptr: NonNull<u8>,
    layout: Layout,
}

impl DevModeBuffer {
    /// Allocate a zeroed buffer of `len` bytes, or of one `DEVMODEW` if `len`
    /// is smaller.
    pub fn zeroed(len: usize) -> Self {
        let layout =
            Layout::from_size_align(len.max(size_of::<DEVMODEW>()), align_of::<DEVMODEW>())
                .expect("DEVMODE buffer size overflows");
        // SAFETY: `layout` has a non-zero size.
        let ptr = NonNull::new(unsafe { alloc_zeroed(layout) })
            .unwrap_or_else(|| handle_alloc_error(layout));
        Self { ptr, layout }
    }

    /// Copy `data` into a new buffer, padded with zeros to one whole
    /// `DEVMODEW` if `data` is shorter.
    pub fn copy_from(data: &[u8]) -> Self {
        let mut buffer = Self::zeroed(data.len());
        buffer[..data.len()].copy_from_slice(data);
        buffer
    }

    pub fn as_dev_mode_ptr(&self) -> *const DEVMODEW {
        self.ptr.as_ptr().cast()
    }

    pub fn as_mut_dev_mode_ptr(&mut self) -> *mut DEVMODEW {
        self.ptr.as_ptr().cast()
    }
}

impl Deref for DevModeBuffer {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        // SAFETY: `ptr` points to `layout.size()` initialized bytes owned by
        // `self`.
        unsafe { slice::from_raw_parts(self.ptr.as_ptr(), self.layout.size()) }
    }
}

impl DerefMut for DevModeBuffer {
    fn deref_mut(&mut self) -> &mut [u8] {
        // SAFETY: `ptr` points to `layout.size()` initialized bytes owned by
        // `self`, and `&mut self` makes the access exclusive.
        unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr(), self.layout.size()) }
    }
}

impl Drop for DevModeBuffer {
    fn drop(&mut self) {
        // SAFETY: `ptr` was allocated with `layout` by `alloc_zeroed`.
        unsafe { dealloc(self.ptr.as_ptr(), self.layout) }
    }
}

impl From<DevModeBuffer> for Vec<u8> {
    /// Copy the bytes into a `Vec<u8>`.
    ///
    /// The allocation cannot be reused: a `Vec<u8>` frees its memory with the
    /// alignment of `u8`, and memory must be freed with the layout that
    /// allocated it.
    fn from(buffer: DevModeBuffer) -> Self {
        buffer.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::DevModeBuffer;
    use std::mem::size_of;
    use windows::Win32::Graphics::Gdi::DEVMODEW;

    #[test]
    fn zeroed_holds_at_least_one_dev_mode() {
        let small = DevModeBuffer::zeroed(0);
        assert_eq!(small.len(), size_of::<DEVMODEW>());
        assert!(small.as_ptr().cast::<DEVMODEW>().is_aligned());
        assert!(small.iter().all(|&byte| byte == 0));

        let large = DevModeBuffer::zeroed(9000);
        assert_eq!(large.len(), 9000);
        assert!(large.as_ptr().cast::<DEVMODEW>().is_aligned());
        assert!(large.iter().all(|&byte| byte == 0));
    }

    #[test]
    fn copy_from_keeps_the_data_and_pads_with_zeros() {
        let data: Vec<u8> = (1..=100).collect();
        let copy = DevModeBuffer::copy_from(&data[1..]);
        assert!(copy.as_ptr().cast::<DEVMODEW>().is_aligned());
        assert_eq!(copy.len(), size_of::<DEVMODEW>());
        assert_eq!(&copy[..data.len() - 1], &data[1..]);
        assert!(copy[data.len() - 1..].iter().all(|&byte| byte == 0));
    }
}
