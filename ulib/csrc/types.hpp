#pragma once

#include <cstdint>
#include <cassert>

typedef int8_t i8;
typedef int16_t i16;
typedef int32_t i32;
typedef int64_t i64;
typedef uint8_t u8;
typedef uint16_t u16;
typedef uint32_t u32;
typedef uint64_t u64;

typedef intptr_t isize;
typedef uintptr_t usize;

const u32 u32_MAX = ~(u32)0;
const usize usize_MAX = ~(usize)0;
const u64 u64_MAX = ~(u64)0;

typedef float f32;
typedef double f64;

#ifdef __NVCC__
#include <math_constants.h>
#define f32_NAN nanf("")
#define f64_NAN nan("")
#define f32_INFINITY INFINITY
#define f64_INFINITY INFINITY
#else
#include <limits>
const f32 f32_NAN = ::std::numeric_limits<f32>::quiet_NaN();
const f64 f64_NAN = ::std::numeric_limits<f64>::quiet_NaN();
const f32 f32_INFINITY = ::std::numeric_limits<f32>::infinity();
const f64 f64_INFINITY = ::std::numeric_limits<f64>::infinity();
#endif
