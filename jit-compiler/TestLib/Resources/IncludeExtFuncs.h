#ifndef RUSTMODLICA_TEST_EXT_FUNCS_H
#define RUSTMODLICA_TEST_EXT_FUNCS_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Test function: sum array elements
/// ABI: double rustmodlica_sum_array(const double* arr, double size)
double rustmodlica_sum_array(const double* arr, double size);

/// Test function: print string and return 1.0
/// ABI: double rustmodlica_print_string(const char* msg)
double rustmodlica_print_string(const char* msg);

#ifdef __cplusplus
}
#endif

#endif /* RUSTMODLICA_TEST_EXT_FUNCS_H */
