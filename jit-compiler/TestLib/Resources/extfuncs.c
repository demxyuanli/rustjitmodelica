/**
 * External functions for testing JIT array and string ABI support
 * Compile with: cl /LD extfuncs.c /Fe:rustmodlica_test_extfuncs.dll
 */

#include <stdio.h>
#include "IncludeExtFuncs.h"

/// Sum array elements
double rustmodlica_sum_array(const double* arr, double size) {
    double sum = 0.0;
    int n = (int)size;
    for (int i = 0; i < n; i++) {
        sum += arr[i];
    }
    return sum;
}

/// Print string and return 1.0
double rustmodlica_print_string(const char* msg) {
    printf("[External] %s\n", msg);
    return 1.0;
}
