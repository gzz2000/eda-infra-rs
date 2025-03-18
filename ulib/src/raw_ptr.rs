//! Wrapper of raw pointers that exports universal pointer interface.
//!
//! This is essential in the interaction between C-FFIs and universal
//! functions.
//!
//! The pointer wrappers can only be created unsafely, because the
//! safety of pointers should be ensured outside.

use super::*;

/// A raw immutable pointer wrapper associated with a device.
#[derive(Debug, Copy, Clone)]
pub struct RawUPtr<T: UniversalCopy> {
    ptr: *const T,
    device: Device
}

/// A raw mutable pointer wrapper associated with a device.
#[derive(Debug)]
pub struct RawUPtrMut<T: UniversalCopy> {
    ptr: *mut T,
    device: Device
}

impl<T: UniversalCopy> RawUPtr<T> {
    #[inline]
    pub unsafe fn new(ptr: *const T, device: Device) -> Self {
        RawUPtr { ptr, device }
    }
}

impl<T: UniversalCopy> RawUPtrMut<T> {
    #[inline]
    pub unsafe fn new(ptr: *mut T, device: Device) -> Self {
        RawUPtrMut { ptr, device }
    }
}

impl<T: UniversalCopy> AsUPtr<T> for RawUPtr<T> {
    #[inline]
    fn as_uptr(&self, device: Device) -> *const T {
        assert_eq!(device, self.device, "raw uptr not on requested device");
        self.ptr
    }
}

impl<T: UniversalCopy> AsUPtr<T> for RawUPtrMut<T> {
    #[inline]
    fn as_uptr(&self, device: Device) -> *const T {
        assert_eq!(device, self.device, "raw uptr not on requested device");
        self.ptr
    }
}

impl<T: UniversalCopy> AsUPtrMut<T> for RawUPtrMut<T> {
    #[inline]
    fn as_mut_uptr(&mut self, device: Device) -> *mut T {
        assert_eq!(device, self.device, "raw uptr not on requested device");
        self.ptr
    }
}

/// An explicit null universal pointer.
///
/// To create one, use [`NullUPtr::new`].
/// 
/// You should only use it in functions that explicitly accepts
/// null pointers (e.g., to represent optional inputs).
#[non_exhaustive]
#[derive(Debug, Copy, Clone)]
pub struct NullUPtr {}

impl NullUPtr {
    /// Create a null universal pointer.
    ///
    /// It is unsafe because generally functions do not expect one.
    /// Use it only in functions explicitly accepting null pointers.
    pub const unsafe fn new() -> NullUPtr {
        NullUPtr {}
    }

    /// Create reference to a null universal pointer.
    pub const unsafe fn new_ref() -> &'static NullUPtr {
        &NullUPtr {}
    }

    /// Create mutable reference to a null universal pointer.
    pub unsafe fn new_mut() -> &'static mut NullUPtr {
        static mut NULL_UPTR: NullUPtr = NullUPtr {};
        &mut *std::ptr::addr_of_mut!(NULL_UPTR)
    }
}

impl<T: UniversalCopy> AsUPtr<T> for NullUPtr {
    #[inline]
    fn as_uptr(&self, _device: Device) -> *const T {
        std::ptr::null()
    }
}

impl<T: UniversalCopy> AsUPtrMut<T> for NullUPtr {
    #[inline]
    fn as_mut_uptr(&mut self, _device: Device) -> *mut T {
        std::ptr::null_mut()
    }
}
