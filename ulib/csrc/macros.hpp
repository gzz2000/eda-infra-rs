#pragma once

#ifdef __NVCC__
#define __ulib_inline __device__ __host__ __forceinline__
#define __ulib_host_inline inline __attribute__((always_inline))
#define __ulib_device_inline __device__ __forceinline__
#else
#define __ulib_inline inline __attribute__((always_inline))
#define __ulib_host_inline inline __attribute__((always_inline))
#endif
