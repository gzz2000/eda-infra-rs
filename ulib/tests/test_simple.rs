#[allow(unused_imports)]
use ulib::{ UVec, Device, AsUPtr, AsUPtrMut };

#[test]
fn test_simple_cpu() {
    let mut uvt: UVec<i32> = UVec::new_zeroed(100, Device::CPU);
    uvt.as_mut()[0] = 10;
    assert_eq!(uvt.as_mut()[2], 0);
    assert_eq!(uvt.as_mut()[0], 10);
    assert_eq!(uvt.clone().as_mut()[0], 10);
}

#[test]
fn test_realloc_cpu() {
    let mut uvt: UVec<i32> = UVec::new_zeroed(2, Device::CPU);
    uvt.as_mut()[0] = 10;

    unsafe { uvt.resize_uninit_preserve(3, Device::CPU); }
    assert!(uvt.len() == 3);
    assert!(uvt.capacity() >= 3);

    uvt.reserve(100, Device::CPU);
    assert!(uvt.len() == 3);
    assert!(uvt.capacity() >= 103);

    assert_eq!(uvt.as_ref()[0], 10);
    assert_eq!(uvt.as_ref()[1], 0);
    assert_eq!(uvt.clone().as_ref()[1], 0);

    uvt.fill(66, Device::CPU);
    assert_eq!(uvt.as_ref(), &[66, 66, 66]);
}

#[cfg(feature = "cuda")]
#[test]
fn test_simple_cuda() {
    let mut cuvt: UVec<i32> = UVec::new_zeroed(100, Device::CUDA(0));
    cuvt.as_mut();
    for i in 0..*ulib::NUM_CUDA_DEVICES {
        cuvt.as_uptr(Device::CUDA(i as u8));
    }
    for i in 0..*ulib::NUM_CUDA_DEVICES {
        cuvt.as_mut_uptr(Device::CUDA(i as u8));
    }
    for i in 0..*ulib::NUM_CUDA_DEVICES {
        cuvt.as_uptr(Device::CUDA(i as u8));
    }
}

#[cfg(feature = "cuda")]
#[test]
fn test_realloc_cuda() {
    let mut uvt: UVec<i32> = UVec::new_zeroed(2, Device::CPU);
    uvt.as_mut()[0] = 10;

    unsafe { uvt.resize_uninit_preserve(3, Device::CUDA(0)); }
    assert!(uvt.len() == 3);
    assert!(uvt.capacity() >= 3);

    uvt.reserve(10, Device::CUDA(0));
    assert!(uvt.len() == 3);
    assert!(uvt.capacity() >= 13);

    assert_eq!(uvt.as_ref()[0], 10);  // back to cpu.
    assert_eq!(uvt.as_ref()[1], 0);

    unsafe { uvt.resize_uninit_nopreserve(20, Device::CUDA(0)); }
    assert!(uvt.len() == 20);
    assert!(uvt.capacity() >= 20);

    uvt.fill(66, Device::CUDA(0));
    assert_eq!(uvt.as_ref(), &[66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66, 66]);
}

#[test]
fn test_vec_into() {
    let uvt = vec![7, 8, 9, 10];
    let uvt: UVec<u32> = uvt.into();
    assert_eq!(uvt.len(), 4);
    assert_eq!(uvt.capacity(), 4);
    assert_eq!(uvt.as_ref()[3], 10);
    assert_eq!(uvt.as_ref()[0], 7);
}

#[test]
fn test_ptr_copy() {
    let devices_to_test = [
        Device::CPU,
        #[cfg(feature = "cuda")] Device::CUDA(0)
    ];
    for device1 in devices_to_test {
        for device2 in devices_to_test {
            let mut uvec0 = UVec::new_filled(233, 1, device1);
            let uvec1 = UVec::new_filled(666, 1, device2);
            unsafe {
                uvec0.copy_from(device1, &uvec1, device2, 1);
            }
            assert_eq!(uvec0[0], 666);
        }
    }
}
