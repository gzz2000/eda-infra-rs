#pragma once

/// this is an algorithm utility collection for
/// heterogeneous application.

#include "macros.hpp"
#include "types.hpp"
#include <algorithm>

#ifdef __CUDACC__
#include <thrust/execution_policy.h>
#include <thrust/sort.h>
#endif

__ulib_inline usize atomicAddUsize(usize *p, usize add) {
#ifdef __CUDA_ARCH__
  // keep this assertion of 64bit machine here..
  static_assert(sizeof(usize) == sizeof(unsigned long long));
  return atomicAdd((unsigned long long *)p,
                   (unsigned long long)add);
#else
  usize v;
#ifdef _OPENMP
#pragma omp atomic capture
#endif
  v = (*p) += add;
  return v - add;
#endif
}

// https://stackoverflow.com/questions/664014/what-integer-hash-function-are-good-that-accepts-an-integer-hash-key
__ulib_inline u64 hash_u64(u64 x) {
  x ^= 0x1cb8b9d87bc84a70LLU;   // prevent hash(0) = 0
  x = (x ^ (x >> 30)) * 0xbf58476d1ce4e5b9LLU;
  x = (x ^ (x >> 27)) * 0x94d049bb133111ebLLU;
  x = x ^ (x >> 31);
  return x;
}

// written by GPT-4
template<typename RandomIter>
void _quicksort_omp_launcher(RandomIter first, RandomIter last) {
  if(!(first < last)) return;
  usize len = last - first;
  if(len < 100) {
    std::sort(first, last);
    return;
  }
  usize swap_pivot_id = hash_u64(len) % len;
  if(swap_pivot_id != len - 1) {
    std::iter_swap(first + swap_pivot_id, last - 1);
  }
  auto pivot = *(last - 1);
  auto i = first - 1;

  for (auto j = first; j < last - 1; ++j) {
    // when equals, split evenly.
    if (*j < pivot || (!(pivot < *j) && ((j - first) & 1))) {
      ++i;
      std::iter_swap(i, j);
    }
  }
  std::iter_swap(i + 1, last - 1);
  auto pi = i + 1;

#ifdef _OPENMP
#pragma omp task
#endif
  _quicksort_omp_launcher(first, pi);

#ifdef _OPENMP
#pragma omp task
#endif
  _quicksort_omp_launcher(pi + 1, last);
}

template<typename RandomIter>
void par_quicksort_cpu(RandomIter first, RandomIter last) {
#ifdef _OPENMP
#pragma omp parallel
#pragma omp single
#endif
  _quicksort_omp_launcher(first, last);
}

#ifdef __CUDACC__
template<typename RandomIter>
void par_quicksort_cuda(RandomIter first, RandomIter last) {
  thrust::sort(thrust::device, first, last);
}
#endif
