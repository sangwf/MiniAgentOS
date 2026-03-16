#ifndef MINI_OS_STDINT_H
#define MINI_OS_STDINT_H
#include <stddef.h>

typedef unsigned char uint8_t;
typedef unsigned short uint16_t;
typedef unsigned int uint32_t;
typedef unsigned long long uint64_t;
typedef signed char int8_t;
typedef signed short int16_t;
typedef signed int int32_t;
typedef signed long long int64_t;

typedef unsigned long uintptr_t;
typedef long intptr_t;

#define SIZE_MAX 18446744073709551615UL

#endif
