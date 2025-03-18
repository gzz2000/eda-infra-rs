//! Universal vector-like array storage [`UVec`].

use super::*;
use std::sync::Mutex;
use std::hash::{ Hash, Hasher };
use std::ops::{ Deref, DerefMut, Index, IndexMut };
use std::fmt;
use std::cell::UnsafeCell;
use bytemuck::Zeroable;

#[cfg(feature = "cuda")]
use cust::memory::{ DeviceBuffer, CopyDestination };
#[cfg(feature = "cuda")]
use cust::context::{ Context, CurrentContext };

/// Universal vector-like array storage.
///
/// `UVec` is thread-safe (`Send` + `Sync`). Specifically, its
/// read-only reference can be shared across different threads.
/// This is nontrivial because a read in `UVec` might schedule
/// a copy across device.
pub struct UVec<T: UniversalCopy>(UnsafeCell<UVecInternal<T>>);

unsafe impl<T: UniversalCopy> Sync for UVec<T> {}

impl<T: UniversalCopy> UVec<T> {
    fn get_intl_mut(&mut self) -> &mut UVecInternal<T> {
        self.0.get_mut()
    }

    fn get_intl(&self) -> &UVecInternal<T> {
        unsafe { &*self.0.get() }
    }

    unsafe fn get_intl_mut_unsafe(&self) -> &mut UVecInternal<T> {
        unsafe { &mut *self.0.get() }
    }
}

/// defines the reallocation heuristic. current we allocate 50\% more.
#[inline]
fn realloc_heuristic(new_len: usize) -> usize {
    (new_len as f64 * 1.5).round() as usize
}

/// The unsafe cell-wrapped internal.
struct UVecInternal<T: UniversalCopy> {
    data_cpu: Option<Box<[T]>>,
    #[cfg(feature = "cuda")]
    data_cuda: [Option<DeviceBuffer<T>>; MAX_NUM_CUDA_DEVICES],
    /// A flag array recording the data presence and dirty status.
    /// A true entry means the data is valid on that device.
    valid_flag: [bool; MAX_DEVICES],
    /// Read locks for all devices
    ///
    /// This will not be locked for any operation originating
    /// from a write access -- no need to do so because Rust
    /// guarantees exclusive mutable reference.
    ///
    /// This will not be locked for readonly reference as long as
    /// our interested device is already ready for read (valid)
    /// -- no need to do so because Rust guarantees no mutation
    /// operation ever possible when a read-only reference
    /// is alive.
    ///
    /// This will ONLY be locked when a copy across device
    /// need to be launched with a read-only reference.
    /// The lock, in this case, is also per receiver device.
    read_locks: [Mutex<()>; MAX_DEVICES],
    /// the length of content
    len: usize,
    /// the length of buffer
    capacity: usize,
}

impl<T: UniversalCopy + fmt::Debug> fmt::Debug for UVec<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.len() == 0 {
            return write!(f, "empty uvec")
        }
        let slice = self.as_ref();
        write!(f, "uvec[{}] = [", slice.len())?;
        for (i, e) in slice.iter().enumerate() {
            if i != 0 {
                write!(f, ", ")?;
            }
            if f.alternate() {
                write!(f, "{:#?}", e)?;
            }
            else {
                write!(f, "{:?}", e)?;
            }
        }
        write!(f, "]")
    }
}

impl<T: UniversalCopy> Default for UVec<T> {
    #[inline]
    fn default() -> Self {
        Self(UnsafeCell::new(UVecInternal {
            data_cpu: None,
            #[cfg(feature = "cuda")]
            data_cuda: Default::default(),
            valid_flag: [false; MAX_DEVICES],
            read_locks: Default::default(),
            len: 0,
            capacity: 0
        }))
    }
}

impl<T: UniversalCopy> From<Box<[T]>> for UVec<T> {
    #[inline]
    fn from(b: Box<[T]>) -> UVec<T> {
        let len = b.len();
        let mut valid_flag = [false; MAX_DEVICES];
        valid_flag[Device::CPU.to_id()] = true;
        Self(UnsafeCell::new(UVecInternal {
            data_cpu: Some(b),
            #[cfg(feature = "cuda")]
            data_cuda: Default::default(),
            valid_flag,
            read_locks: Default::default(),
            len,
            capacity: len
        }))
    }
}

impl<T: UniversalCopy> UVec<T> {
    /// Create a UVec by cloning from a universal pointer.
    ///
    /// Safety: the given pointer must be valid for `len` elements,
    /// and can be queried from the specific device.
    #[inline]
    pub unsafe fn from_uptr_cloned(
        ptr: impl AsUPtr<T>, len: usize, device: Device
    ) -> UVec<T> {
        let mut uvec = UVec::new_uninitialized(len, device);
        uvec.copy_from(device, ptr, device, len);
        uvec
    }
}

impl<T: UniversalCopy> From<Vec<T>> for UVec<T> {
    #[inline]
    fn from(v: Vec<T>) -> UVec<T> {
        v.into_boxed_slice().into()
    }
}

impl<T: UniversalCopy> FromIterator<T> for UVec<T> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Vec::from_iter(iter).into()
    }
}

impl<T: UniversalCopy> From<UVec<T>> for Box<[T]> {
    #[inline]
    fn from(mut v: UVec<T>) -> Box<[T]> {
        v.schedule_device_read(Device::CPU);
        v.get_intl_mut().data_cpu.take().unwrap()
    }
}

impl<T: UniversalCopy> From<UVec<T>> for Vec<T> {
    #[inline]
    fn from(v: UVec<T>) -> Vec<T> {
        Box::<[T]>::from(v).into()
    }
}

#[cfg(feature = "rayon")]
mod uvec_rayon {
    use super::*;
    use rayon::prelude::*;
    
    impl<'i, T: UniversalCopy + Sync + 'i> IntoParallelIterator for &'i UVec<T> {
        type Iter = <&'i [T] as IntoParallelIterator>::Iter;
        type Item = &'i T;
        
        #[inline]
        fn into_par_iter(self) -> Self::Iter {
            self.as_ref().into_par_iter()
        }
    }

    impl<'i, T: UniversalCopy + Send + 'i> IntoParallelIterator for &'i mut UVec<T> {
        type Iter = <&'i mut [T] as IntoParallelIterator>::Iter;
        type Item = &'i mut T;
        
        #[inline]
        fn into_par_iter(self) -> Self::Iter {
            self.as_mut().into_par_iter()
        }
    }

    impl<T: UniversalCopy + Send> IntoParallelIterator for UVec<T> {
        type Iter = <Vec<T> as IntoParallelIterator>::Iter;
        type Item = T;
        
        #[inline]
        fn into_par_iter(self) -> Self::Iter {
            Vec::<T>::from(self).into_par_iter()
        }
    }

    impl<T: UniversalCopy + Send> FromParallelIterator<T> for UVec<T> {
        #[inline]
        fn from_par_iter<I: IntoParallelIterator<Item = T>>(
            par_iter: I
        ) -> Self {
            Vec::from_par_iter(par_iter).into()
        }
    }
}

impl<T: UniversalCopy + Zeroable> UVecInternal<T> {
    /// private function to allocate space for one device.
    ///
    /// Guaranteed to only modify the buffer and the validity
    /// bit of the specified device.
    /// (which is useful in the safety of read-schedule
    /// interior mutability.)
    #[inline]
    fn alloc_zeroed(&mut self, device: Device) {
        use Device::*;
        match device {
            CPU => {
                use std::alloc;
                self.data_cpu = Some(unsafe {
                    let ptr = alloc::alloc_zeroed(
                        alloc::Layout::array::<T>(
                            self.capacity
                        ).unwrap()) as *mut T;
                    Box::from_raw(
                        core::ptr::slice_from_raw_parts_mut(
                            ptr, self.len))
                    // Box::new_zeroed_slice(sz).assume_init()
                });
            },
            #[cfg(feature = "cuda")]
            CUDA(c) => {
                let _context = Context::new(
                    CUDA_DEVICES[c as usize].0).unwrap();
                self.data_cuda[c as usize] =
                    Some(DeviceBuffer::zeroed(self.capacity)
                         .unwrap());
            }
        }
    }
}

#[inline]
unsafe fn alloc_cpu_uninit<T: UniversalCopy>(
    sz: usize
) -> Box<[T]> {
    use std::alloc;
    let ptr = alloc::alloc(alloc::Layout::array::<T>(sz).unwrap())
        as *mut T;
    Box::from_raw(core::ptr::slice_from_raw_parts_mut(ptr, sz))
}

#[cfg(feature = "cuda")]
#[inline]
unsafe fn alloc_cuda_uninit<T: UniversalCopy>(
    sz: usize, dev: u8
) -> DeviceBuffer<T> {
    let _context = Context::new(CUDA_DEVICES[dev as usize].0)
        .unwrap();
    DeviceBuffer::uninitialized(sz).unwrap()
}

impl<T: UniversalCopy> UVecInternal<T> {
    /// private function to allocate space for one device.
    ///
    /// Guaranteed to only modify the buffer and the validity
    /// bit of the specified device.
    /// (which is useful in the safety of read-schedule
    /// interior mutability.)
    #[inline]
    unsafe fn alloc_uninitialized(&mut self, device: Device) {
        use Device::*;
        match device {
            CPU => {
                self.data_cpu = Some(alloc_cpu_uninit(
                    self.capacity));
            },
            #[cfg(feature = "cuda")]
            CUDA(c) => {
                self.data_cuda[c as usize] = Some(
                    alloc_cuda_uninit(self.capacity, c));
            }
        }
    }
    
    /// private function to get one device with valid data
    #[inline]
    fn device_valid(&self) -> Option<Device> {
        self.valid_flag.iter().enumerate().find(|(_i, v)| **v)
            .map(|(i, _v)| Device::from_id(i))
    }
    
    #[inline]
    fn drop_all_buf(&mut self) {
        self.data_cpu = None;
        #[cfg(feature = "cuda")]
        for d in &mut self.data_cuda {
            *d = None;
        }
    }

    #[inline]
    unsafe fn realloc_uninit_nopreserve(&mut self, device: Device) {
        self.drop_all_buf();
        if self.capacity > 10000000 {
            clilog::debug!("large realloc: capacity {}",
                           self.capacity);
        }
        self.alloc_uninitialized(device);
        self.valid_flag.fill(false);
        self.valid_flag[device.to_id()] = true;
    }
    
    #[inline]
    unsafe fn realloc_uninit_preserve(&mut self, device: Device) {
        use Device::*;
        match device {
            CPU => {
                let old = self.data_cpu.take().unwrap();
                self.drop_all_buf();
                self.alloc_uninitialized(device);
                self.data_cpu.as_mut().unwrap()[..self.len]
                    .copy_from_slice(&old[..self.len]);
            },
            #[cfg(feature = "cuda")]
            CUDA(c) => {
                let _context = CUDA(c).get_context();
                let c = c as usize;
                let old = self.data_cuda[c].take().unwrap();
                self.drop_all_buf();
                self.alloc_uninitialized(device);
                self.data_cuda[c].as_mut().unwrap().index(..self.len)
                    .copy_from(&old.index(..self.len))
                    .unwrap();
            }
        }
        self.valid_flag.fill(false);
        self.valid_flag[device.to_id()] = true;
    }

    /// schedule a device to make its data available.
    ///
    /// Guaranteed to only modify the buffer and the validity
    /// bit of the specified device.
    /// (which is useful in the safety of read-schedule
    /// interior mutability.)
    #[inline]
    fn schedule_device_read(&mut self, device: Device) {
        if self.valid_flag[device.to_id()] {
            return
        }
        use Device::*;
        let is_none = match device {
            CPU => self.data_cpu.is_none(),
            #[cfg(feature = "cuda")]
            CUDA(c) => self.data_cuda[c as usize].is_none()
        };
        if is_none {
            unsafe { self.alloc_uninitialized(device); }
        }
        if self.capacity == 0 {
            return
        }
        let device_valid = self.device_valid().expect("no valid dev");
        match (device_valid, device) {
            (CPU, CPU) => {},
            #[cfg(feature = "cuda")]
            (CPU, CUDA(c)) => {
                let _context = CUDA(c).get_context();
                let c = c as usize;
                self.data_cuda[c].as_mut().unwrap().index(..self.len)
                    .copy_from(
                        &self.data_cpu.as_ref().unwrap()[..self.len]
                    ).unwrap();
            },
            #[cfg(feature = "cuda")]
            (CUDA(c), CPU) => {
                let _context = CUDA(c).get_context();
                let c = c as usize;
                self.data_cuda[c].as_ref().unwrap().index(..self.len)
                    .copy_to(
                        &mut self.data_cpu.as_mut().unwrap()[..self.len]
                    ).unwrap();
                CurrentContext::synchronize().unwrap();
            },
            #[cfg(feature = "cuda")]
            (CUDA(c1), CUDA(c2)) => {
                let _context = CUDA(c2).get_context();
                let (c1, c2) = (c1 as usize, c2 as usize);
                assert_ne!(c1, c2);
                // unsafe is used to access one mutable element.
                // safety guaranteed by the above `assert_ne!`.
                let c2_mut = unsafe {
                    &mut *(self.data_cuda[c2].as_mut().unwrap()
                           as *const DeviceBuffer<T>
                           as *mut DeviceBuffer<T>)
                };
                self.data_cuda[c1].as_ref().unwrap().index(..self.len)
                    .copy_to(
                        &mut c2_mut.index(..self.len)
                    ).unwrap();
            }
        }
        self.valid_flag[device.to_id()] = true;
    }
}

impl<T: UniversalCopy> UVec<T> {
    /// schedule a device to make its data available.
    ///
    /// Guaranteed to only modify the buffer and the validity
    /// bit of the specified device.
    /// (which is useful in the safety of read-schedule
    /// interior mutability.)
    #[inline]
    fn schedule_device_read(&mut self, device: Device) {
        self.get_intl_mut().schedule_device_read(device);
    }

    /// schedule a device to make its data available
    /// THROUGH a read-only reference.
    ///
    /// will acquire a lock if it is necessary.
    /// If you have mutable reference, use the lock-free
    /// `schedule_device_read` instead.
    #[inline]
    fn schedule_device_read_ro(&self, device: Device) {
        // safety guaranteed by the lock, and by the
        // guarantee of `schedule_device_read` that only
        // writes to fields related to the specified device.
        let intl = unsafe {
            self.get_intl_mut_unsafe()
        };
        let intl_erased = unsafe {
            &mut *(self.get_intl_mut_unsafe() as *mut UVecInternal<T>)
        };
        if intl.valid_flag[device.to_id()] {
            return
        }
        let locked = intl.read_locks[device.to_id()]
            .lock().unwrap();
        intl_erased.schedule_device_read(device);
        drop(locked);
    }

    /// schedule a device write. invalidates all other ranges.
    #[inline]
    fn schedule_device_write(&mut self, device: Device) {
        let intl = self.get_intl_mut();
        if !intl.valid_flag[device.to_id()] {
            intl.schedule_device_read(device);
        }
        // only this is valid.
        intl.valid_flag[..].fill(false);
        intl.valid_flag[device.to_id()] = true;
    }

    #[inline]
    pub fn get(&self, idx: usize) -> T {
        use Device::*;
        let intl = self.get_intl();
        match intl.device_valid().unwrap() {
            CPU => intl.data_cpu.as_ref().unwrap()[idx],
            #[cfg(feature = "cuda")]
            CUDA(c) => {
                let _context = CUDA(c).get_context();
                let mut ret: [T; 1] = unsafe {
                    std::mem::MaybeUninit::uninit().assume_init()
                };
                intl.data_cuda[c as usize].as_ref().unwrap()
                    .index(idx)
                    .copy_to(&mut ret)
                    .unwrap();
                CurrentContext::synchronize().unwrap();
                ret[0]
            }
        }
    }
}

impl<T: UniversalCopy + Zeroable> UVec<T> {
    /// Create a new zeroed universal vector with specific size and
    /// capacity;
    #[inline]
    pub fn new_zeroed_with_capacity(
        len: usize, capacity: usize, device: Device
    ) -> UVec<T> {
        let mut v: UVec<T> = Default::default();
        let intl = v.get_intl_mut();
        assert!(len <= capacity);
        intl.len = len;
        intl.capacity = capacity;
        intl.alloc_zeroed(device);
        intl.valid_flag[device.to_id()] = true;
        v
    }

    /// Create a new zeroed universal vector with specific size.
    #[inline]
    pub fn new_zeroed(len: usize, device: Device) -> UVec<T> {
        Self::new_zeroed_with_capacity(len, len, device)
    }
}

impl<T: UniversalCopy> UVec<T> {
    /// Get length (size) of this universal vector.
    #[inline]
    pub fn len(&self) -> usize {
        self.get_intl().len
    }

    /// Returns `true` if this universal vector has a length of 0.
    ///
    /// Empty uvec can have no valid devices, and will return nullptr on
    /// `AsRef` or [`AsUPtr`] calls.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.get_intl().len == 0
    }
    
    /// Get capacity of this vector.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.get_intl().capacity
    }

    /// New empty vector.
    ///
    /// This should only be used as a placeholder. it allocates
    /// nothing. will panic if you try to get any pointer from it.
    #[inline]
    pub fn new() -> UVec<T> {
        Default::default()
    }

    /// Create a new uninitialized universal vector with
    /// specific size and capacity.
    #[inline]
    pub unsafe fn new_uninitialized_with_capacity(
        len: usize, capacity: usize, device: Device
    ) -> UVec<T> {
        let mut v: UVec<T> = Default::default();
        let intl = v.get_intl_mut();
        assert!(len <= capacity);
        intl.len = len;
        intl.capacity = capacity;
        intl.alloc_uninitialized(device);
        intl.valid_flag[device.to_id()] = true;
        v
    }

    /// Create a new uninitialized universal vector with
    /// specific size.
    #[inline]
    pub unsafe fn new_uninitialized(
        len: usize, device: Device
    ) -> UVec<T> {
        Self::new_uninitialized_with_capacity(len, len, device)
    }

    /// Create a new zero length universal vector with specific
    /// initial capacity.
    #[inline]
    pub fn with_capacity(
        capacity: usize, device: Device
    ) -> UVec<T> {
        unsafe {
            Self::new_uninitialized_with_capacity(0, capacity, device)
        }
    }

    /// Force set the length of this vector.
    ///
    /// this is a low-level operation that does not reallocate.
    /// safe only when the new length does not exceed current capacity
    /// and the new visible elements (if any) are not uninitialized.
    ///
    /// See also `std::vec::Vec::set_len`.
    #[inline]
    pub unsafe fn set_len(&mut self, new_len: usize) {
        let intl = self.get_intl_mut();
        assert!(new_len <= intl.capacity);
        intl.len = new_len;
    }

    /// Reserves capacity for at least `additional` more elements
    /// to be inserted in the given `UVec<T>`.
    ///
    /// The collection may reserve more space to speculatively avoid
    /// frequent reallocations. After calling reserve, capacity will be
    /// greater than or equal to self.len() + `additional`.
    /// Does nothing if capacity is already sufficient.
    ///
    /// See also `std::vec::Vec::reserve`. we have an additional arg
    /// `device` specifying, when an re-allocation is necessary, which
    /// device's data needs preserving (often means immediate use).
    ///
    /// The `reserve` and `resize_uninit_[no]preserve` are two distinct
    /// methodologies of reallocation (len-based or capacity-based).
    /// You can choose one at your convenience.
    #[inline]
    pub fn reserve(&mut self, additional: usize, device: Device) {
        let intl = self.get_intl_mut();
        if intl.len + additional <= intl.capacity {
            return
        }
        intl.capacity = realloc_heuristic(intl.len + additional);
        unsafe { intl.realloc_uninit_preserve(device); }
    }

    /// Resize the universal vector, but do **not** preserve the
    /// original content.
    /// The potential new elements are **uninitialized**.
    ///
    /// If the current capacity is sufficient, we do not need to
    /// reallocate or do anything else. We just mark the desired
    /// device as valid.
    ///
    /// If the current capacity is insufficient, a reallocation
    /// is needed and all current allocations are dropped.
    /// (we maintain the invariant that all allocated buffers for
    /// all devices must all have the same length (= capacity).)
    #[inline]
    pub unsafe fn resize_uninit_nopreserve(&mut self, len: usize, device: Device) {
        let intl = self.get_intl_mut();
        if intl.capacity < len {
            intl.capacity = realloc_heuristic(len);
            intl.realloc_uninit_nopreserve(device);
        }
        intl.len = len;
    }

    /// Resize the universal vector, and preserve all the
    /// original content.
    /// The potential new elements are **uninitialized**.
    #[inline]
    pub unsafe fn resize_uninit_preserve(&mut self, len: usize, device: Device) {
        if self.get_intl().len != 0 {
            self.schedule_device_read(device);
        }
        let intl = self.get_intl_mut();
        if intl.capacity < len {
            intl.capacity = realloc_heuristic(len);
            intl.realloc_uninit_preserve(device);
        }
        intl.len = len;
        intl.valid_flag.fill(false);
        intl.valid_flag[device.to_id()] = true;
    }

    #[inline]
    pub fn fill(&mut self, value: T, device: Device) {
        self.fill_len(value, self.len(), device);
    }

    #[inline]
    pub fn new_filled(value: T, len: usize, device: Device) -> UVec<T> {
        let mut v = unsafe { Self::new_uninitialized(len, device) };
        v.fill(value, device);
        v
    }
}

impl<T: UniversalCopy> AsRef<[T]> for UVec<T> {
    /// Get a CPU slice reference.
    /// 
    /// This COULD fail, actually, when we need to copy from
    /// a GPU value to CPU.
    /// This violates the guideline but we have no choice.
    ///
    /// It will lock only when a copy is needed.
    #[inline]
    fn as_ref(&self) -> &[T] {
        self.schedule_device_read_ro(Device::CPU);
        let intl = self.get_intl();
        &intl.data_cpu.as_ref().unwrap()[..intl.len]
    }
}

impl<T: UniversalCopy> AsMut<[T]> for UVec<T> {
    /// Get a mutable CPU slice reference.
    /// 
    /// This COULD fail, actually, when we need to copy from
    /// a GPU value to CPU.
    /// This violates the guideline but we have no choice.
    ///
    /// It is lock-free.
    #[inline]
    fn as_mut(&mut self) -> &mut [T] {
        self.schedule_device_write(Device::CPU);
        let intl = self.get_intl_mut();
        &mut intl.data_cpu.as_mut().unwrap()[..intl.len]
    }
}

impl<T: UniversalCopy> Deref for UVec<T> {
    type Target = [T];
    /// `Deref` is now implemented for `UVec` to let you
    /// use it transparently.
    ///
    /// Internally it may fail because it might schedule a
    /// inter-device copy to make the data available on CPU.
    /// But it is thread-safe.
    #[inline]
    fn deref(&self) -> &[T] {
        self.as_ref()
    }
}

impl<T: UniversalCopy> DerefMut for UVec<T> {
    /// `Deref` is now implemented for `UVec` to let you
    /// use it transparently.
    ///
    /// Internally it may fail because it might schedule a
    /// inter-device copy to make the data available on CPU.
    /// But it is thread-safe.
    #[inline]
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut()
    }
}

impl<T: UniversalCopy, I> Index<I> for UVec<T> where [T]: Index<I> {
    type Output = <[T] as Index<I>>::Output;
    #[inline]
    fn index(&self, i: I) -> &Self::Output {
        self.as_ref().index(i)
    }
}

impl<T: UniversalCopy, I> IndexMut<I> for UVec<T> where [T]: IndexMut<I> {
    #[inline]
    fn index_mut(&mut self, i: I) -> &mut Self::Output {
        self.as_mut().index_mut(i)
    }
}

impl<T: UniversalCopy> IntoIterator for UVec<T> {
    type Item = T;
    type IntoIter = <Vec<T> as IntoIterator>::IntoIter;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        Vec::from(self).into_iter()
    }
}

impl<'i, T: UniversalCopy> IntoIterator for &'i UVec<T> {
    type Item = &'i T;
    type IntoIter = <&'i [T] as IntoIterator>::IntoIter;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.as_ref().into_iter()
    }
}

impl<T: UniversalCopy> AsUPtr<T> for UVec<T> {
    #[inline]
    fn as_uptr(&self, device: Device) -> *const T {
        if self.capacity() == 0 {
            return std::ptr::null()
        }
        self.schedule_device_read_ro(device);
        let intl = self.get_intl();
        use Device::*;
        match device {
            CPU => intl.data_cpu.as_ref().unwrap().as_ptr(),
            #[cfg(feature = "cuda")]
            CUDA(c) => intl.data_cuda[c as usize].as_ref().unwrap()
                .as_device_ptr().as_ptr()
        }
    }
}

impl<T: UniversalCopy> AsUPtrMut<T> for UVec<T> {
    #[inline]
    fn as_mut_uptr(&mut self, device: Device) -> *mut T {
        if self.capacity() == 0 {
            return std::ptr::null_mut()
        }
        self.schedule_device_write(device);
        let intl = self.get_intl_mut();
        use Device::*;
        match device {
            CPU => intl.data_cpu.as_mut().unwrap().as_mut_ptr(),
            #[cfg(feature = "cuda")]
            CUDA(c) => intl.data_cuda[c as usize].as_mut().unwrap()
                .as_device_ptr().as_mut_ptr()
        }
    }
}

// although convenient, below gets in the way of automatic type inference.

// impl<T: UniversalCopy, const N: usize> AsUPtr<T> for UVec<[T; N]> {
//     /// convenient way to get flattened pointer
//     #[inline]
//     fn as_uptr(&self, device: Device) -> *const T {
//         AsUPtr::<[T; N]>::as_uptr(self, device) as *const T
//     }
// }

// impl<T: UniversalCopy, const N: usize> AsUPtrMut<T> for UVec<[T; N]> {
//     /// convenient way to get flattened pointer
//     #[inline]
//     fn as_mut_uptr(&mut self, device: Device) -> *mut T {
//         AsUPtrMut::<[T; N]>::as_mut_uptr(self, device) as *mut T
//     }
// }

impl<T: UniversalCopy + Hash> Hash for UVec<T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state)
    }
}

impl<T: UniversalCopy, U: UniversalCopy> PartialEq<UVec<U>> for UVec<T>
    where T: PartialEq<U>
{
    #[inline]
    fn eq(&self, other: &UVec<U>) -> bool {
        if self.len() != other.len() {
            return false
        }
        if self.is_empty() {
            return true
        }
        self.as_ref() == other.as_ref()
    }
}

impl<T: UniversalCopy + Eq> Eq for UVec<T> { }

impl<T: UniversalCopy> Clone for UVecInternal<T> {
    fn clone(&self) -> Self {
        let valid_flag = self.valid_flag.clone();
        let data_cpu = match valid_flag[Device::CPU.to_id()] {
            true => self.data_cpu.clone(),
            false => None
        };
        #[cfg(feature = "cuda")]
        let data_cuda = unsafe {
            let mut data_cuda: [Option<DeviceBuffer<T>>; MAX_NUM_CUDA_DEVICES] = Default::default();
            for i in 0..MAX_NUM_CUDA_DEVICES {
                if valid_flag[Device::CUDA(i as u8).to_id()] {
                    let _context = Device::CUDA(i as u8).get_context();
                    let dbuf = alloc_cuda_uninit(self.capacity, i as u8);
                    self.data_cuda[i].as_ref().unwrap().index(..self.len)
                        .copy_to(&mut dbuf.index(..self.len))
                        .unwrap();
                    data_cuda[i] = Some(dbuf);
                }
            }
            data_cuda
        };
        UVecInternal {
            data_cpu,
            #[cfg(feature = "cuda")] data_cuda,
            valid_flag,
            read_locks: Default::default(),
            len: self.len,
            capacity: self.capacity
        }
    }
}

impl<T: UniversalCopy> Clone for UVec<T> {
    fn clone(&self) -> Self {
        Self(UnsafeCell::new(self.get_intl().clone()))
    }
}
