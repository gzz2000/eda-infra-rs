// general memset with arbitrary element size

#include "types.hpp"

extern "C"
void ulib_fill_memory_1byte_cpu(u8 *a, usize len, u8 val) {
#pragma omp parallel for
  for(usize i = 0; i < len; ++i) a[i] = val;
}

extern "C"
void ulib_fill_memory_2byte_cpu(u16 *a, usize len, u16 val) {
#pragma omp parallel for
  for(usize i = 0; i < len; ++i) a[i] = val;
}

extern "C"
void ulib_fill_memory_4byte_cpu(u32 *a, usize len, u32 val) {
#pragma omp parallel for
  for(usize i = 0; i < len; ++i) a[i] = val;
}

extern "C"
void ulib_fill_memory_8byte_cpu(u64 *a, usize len, u64 val) {
#pragma omp parallel for
  for(usize i = 0; i < len; ++i) a[i] = val;
}

extern "C"
void ulib_fill_memory_anybyte_cpu(u8 *a, usize len, const u8 *val, usize size) {
#pragma omp parallel for
  for(usize i = 0; i < len; ++i) {
    for(usize j = 0; j < size; ++j) {
      a[i * size + j] = val[j];
    }
  }
}
