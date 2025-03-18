//! # ulib: General library for universal computing.
//!
//! This library basically implements traits and structs for holding vectors on hosts and different kinds of devices.
//! It is intended to be used with ucc builder which generates wrapper bindings using this library.
//!
//! CUDA support must be manually enabled using the
//! feature `cuda`.

use impl_tools::autoimpl;
use std::rc::Rc;
use std::sync::Arc;

#[allow(unused_imports)]
use lazy_static::lazy_static;

// For our derive macros to refer to cust even when cust is
// not listed as a dependency in our dependent crates.
#[cfg(feature = "cuda")]
pub extern crate cust;

/// The derive macro for types that can be safely
/// bit-copied between all heterogeneous devices.
pub use ulib_derive::UniversalCopy;

// re-export bytemuck Zeroable trait.
pub use bytemuck::Zeroable;

/// The derive macro for zeroable types.
/// see [original docs online](https://docs.rs/zeroable/latest/zeroable/zeroable_docs/index.html).
/// 
pub use ulib_zeroable_derive::Zeroable;

#[cfg(feature = "cuda")]
use cust::memory::DeviceCopy;

/// The maximum number of CUDA devices.
#[cfg(feature = "cuda")]
pub const MAX_NUM_CUDA_DEVICES: usize = 4;
/// The maximum number of devices.
#[cfg(feature = "cuda")]
pub const MAX_DEVICES: usize = MAX_NUM_CUDA_DEVICES + 1;

/// The maximum number of devices.
#[cfg(not(feature = "cuda"))]
pub const MAX_DEVICES: usize = 1;

/// All supported device types.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Device {
    /// The CPU device type.
    CPU,
    /// The CUDA device type with a CUDA device index.
    #[cfg(feature = "cuda")]
    CUDA(u8 /* device id */)
}

/// A RAII device context.
///
/// It may help you redirect computations
/// and memory allocations on specific platforms, e.g., CUDA.
///
/// It can be created using [`Device::get_context`].
/// For CPU device, it does nothing. For CUDA devices, it
/// holds a (RAII) CUDA primary context.
pub struct DeviceContext {
    #[cfg(feature = "cuda")]
    #[allow(dead_code)]
    cuda_context: Option<cust::context::Context>,
}

impl Device {
    #[inline]
    fn to_id(self) -> usize {
        use Device::*;
        match self {
            CPU => 0,
            #[cfg(feature = "cuda")]
            CUDA(c) => {
                assert!((c as usize) < MAX_NUM_CUDA_DEVICES,
                        "invalid cuda device id");
                c as usize + 1
            }
        }
    }
    
    #[inline]
    fn from_id(id: usize) -> Device {
        use Device::*;
        match id {
            0 => CPU,
            #[cfg(feature = "cuda")]
            c @ 1..=MAX_NUM_CUDA_DEVICES => CUDA(c as u8 - 1),
            id @ _ => panic!("device id {} is invalid.", id)
        }
    }

    /// Initializes and returns the platform-related context
    /// of this device.
    #[inline]
    pub fn get_context(self) -> DeviceContext {
        use Device::*;
        match self {
            CPU => DeviceContext {
                #[cfg(feature = "cuda")]
                cuda_context: None
            },
            #[cfg(feature = "cuda")]
            CUDA(c) => DeviceContext {
                cuda_context: Some(cust::context::Context::new(
                    CUDA_DEVICES[c as usize].0).unwrap())
            }
        }
    }

    /// Synchronize all calculations on this device.
    ///
    /// Does nothing for CPU. Calls context synchronize for CUDA.
    #[inline]
    pub fn synchronize(self) {
        use Device::*;
        match self {
            CPU => {},
            #[cfg(feature = "cuda")]
            CUDA(c) => {
                let _context = cust::context::Context::new(
                    CUDA_DEVICES[c as usize].0).unwrap();
                cust::context::CurrentContext::synchronize().unwrap();
            }
        }
    }
}

/// The trait for universally bit-copyable element.
///
/// For cuda build, this is equivalent to `Copy + cust::DeviceCopy`.
/// You can use the derive macro like this:
///
/// ```
/// use ulib::UniversalCopy;
/// 
/// #[derive(UniversalCopy, Clone)]
/// struct Test {
///     a: i32,
///     b: usize
/// }
/// ```
#[cfg(feature = "cuda")]
pub trait UniversalCopy: Copy + DeviceCopy { }
#[cfg(feature = "cuda")]
impl<T: Copy + DeviceCopy> UniversalCopy for T { }

/// Trait for types that can be safely bit-copied between
/// all heterogeneous devices.
///
/// For cpu-only build, this is equivalent to a pure `Copy`.
/// You can use the derive macro like this:
///
/// ```
/// use ulib::UniversalCopy;
/// 
/// #[derive(UniversalCopy, Clone)]
/// struct Test {
///     a: i32,
///     b: usize
/// }
/// ```
#[cfg(not(feature = "cuda"))]
pub trait UniversalCopy: Copy { }
#[cfg(not(feature = "cuda"))]
impl<T: Copy> UniversalCopy for T { }

#[cfg(feature = "cuda")]
lazy_static! {
    /// vector of all devices and their primary contexts.
    ///
    /// the contexts follow the CUDA Driver API, not the runtime API.
    /// all contexts are kept here so they are never deallocated.
    static ref CUDA_DEVICES: Vec<(cust::device::Device, cust::context::Context)> = {
        // initialize the CUDA driver here and only here.
        cust::init(cust::CudaFlags::empty()).unwrap();
        let mut ret = cust::device::Device::devices().unwrap()
            .map(|d| {
                let d = d.unwrap();
                (d, cust::context::Context::new(d).unwrap())
            })
            .collect::<Vec<_>>();
        if ret.len() > MAX_NUM_CUDA_DEVICES as usize {
            clilog::warn!(ULIB_CUDA_TRUNC,
                          "the number of available cuda gpus {} \
                           exceed max supported {}, truncated.",
                          ret.len(), MAX_NUM_CUDA_DEVICES);
            ret.truncate(MAX_NUM_CUDA_DEVICES as usize);
        }
        ret
    };
    
    /// the number of CUDA devices.
    pub static ref NUM_CUDA_DEVICES: usize = CUDA_DEVICES.len();
}

use std::mem::size_of;

mod memfill_ucci {
    use crate as ulib;
    include!(concat!(env!("OUT_DIR"), "/uccbind/memfill.rs"));
}

/// A trait to get raw pointer for any device.
#[autoimpl(for<P: trait + ?Sized> &P, &mut P, Box<P>, Rc<P>, Arc<P>)]
pub trait AsUPtr<T: UniversalCopy> {
    /// Get an immutable raw pointer.
    fn as_uptr(&self, device: Device) -> *const T;
}

/// A trait to get mutable raw pointer for any device.
#[autoimpl(for<P: trait + ?Sized> &mut P, Box<P>)]
pub trait AsUPtrMut<T: UniversalCopy> {
    /// Get a mutable raw pointer.
    fn as_mut_uptr(&mut self, device: Device) -> *mut T;

    /// Fill the memory with given value.
    #[inline]
    fn fill_len(&mut self, value: T, len: usize, device: Device) {
        let mut v = unsafe {UVec::new_uninitialized(1, Device::CPU)};
        v[0] = value;

        macro_rules! match_size {
            ($($num:literal => ($func:ident, $typ:ty)),+,
             _ => $general:expr
            ) => {
                match size_of::<T>() {
                    $($num => {
                        memfill_ucci::$func(
                            unsafe {RawUPtrMut::new(
                                self.as_mut_uptr(device) as *mut $typ, device
                            )},
                            len,
                            unsafe {*(v.as_ptr() as *const $typ)},
                            device
                        );
                    }),+,
                    _ => $general
                }
            }
        }

        match_size!(
            1 => (ulib_fill_memory_1byte, u8),
            2 => (ulib_fill_memory_2byte, u16),
            4 => (ulib_fill_memory_4byte, u32),
            8 => (ulib_fill_memory_8byte, u64),
            _ => {
                memfill_ucci::ulib_fill_memory_anybyte(
                    unsafe {RawUPtrMut::new(
                        self.as_mut_uptr(device) as *mut u8, device
                    )},
                    len,
                    unsafe {RawUPtr::new(
                        v.as_uptr(device) as *const u8, device
                    )},
                    size_of::<T>(),
                    device
                );
            }
        );
    }

    /// Copy `count * sizeof::<T>()` bytes from universal pointer `src`
    /// to `self` on (possibly another) device.
    ///
    /// This is semantically similar to `pointer::copy_from`,
    /// except two device annotations are provided. See safety
    /// instructions there.
    /// The argument orders are the reverse of [`copy`].
    #[inline]
    unsafe fn copy_from(
        &mut self, dest_device: Device,
        src: impl AsUPtr<T>, src_device: Device, count: usize,
    ) {
        copy(src, src_device, self, dest_device, count);
    }
}

/// Copy `count * sizeof::<T>()` bytes from `src` universal pointer
/// to `dest` on (possibly another) device.
///
/// This is semantically similar to [`std::ptr::copy`],
/// except two device annotations are provided. See safety
/// instructions there.
pub unsafe fn copy<T: UniversalCopy>(
    src: impl AsUPtr<T>, src_device: Device,
    mut dest: impl AsUPtrMut<T>, dest_device: Device,
    count: usize
) {
    if count == 0 { return }
    let ptr_dest = dest.as_mut_uptr(dest_device);
    let ptr_src = src.as_uptr(src_device);
    use Device::*;
    #[cfg(feature = "cuda")]
    use cust::{
        memory::{DeviceSlice, DevicePointer, CopyDestination},
        context::CurrentContext,
        sys::CUdeviceptr
    };
    match (dest_device, src_device) {
        (CPU, CPU) => {
            ptr_dest.copy_from(ptr_src, count);
        },
        #[cfg(feature = "cuda")]
        (CPU, CUDA(c)) => {
            let _context = CUDA(c).get_context();
            let device_slice_src = DeviceSlice::from_raw_parts(
                DevicePointer::from_raw(ptr_src as CUdeviceptr),
                count
            );
            let slice_dest = std::slice::from_raw_parts_mut(ptr_dest, count);
            device_slice_src.copy_to(slice_dest).unwrap();
            CurrentContext::synchronize().unwrap();
        },
        #[cfg(feature = "cuda")]
        (CUDA(c), CPU) => {
            let _context = CUDA(c).get_context();
            let mut device_slice_dest = DeviceSlice::from_raw_parts_mut(
                DevicePointer::from_raw(ptr_dest as CUdeviceptr),
                count
            );
            let slice_src = std::slice::from_raw_parts(ptr_src, count);
            device_slice_dest.copy_from(slice_src).unwrap();
        },
        #[cfg(feature = "cuda")]
        (CUDA(c1), CUDA(_c2)) => {
            let _context = CUDA(c1).get_context();
            let mut device_slice_dest = DeviceSlice::<T>::from_raw_parts_mut(
                DevicePointer::from_raw(ptr_dest as CUdeviceptr),
                count
            );
            let device_slice_src = DeviceSlice::from_raw_parts(
                DevicePointer::from_raw(ptr_src as CUdeviceptr),
                count
            );
            device_slice_dest.copy_from(&device_slice_src).unwrap();
        },
    }
}

mod uvec;
pub use uvec::UVec;

mod raw_ptr;
pub use raw_ptr::{ RawUPtr, RawUPtrMut, NullUPtr };

pub mod profile;
