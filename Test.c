#include <stdio.h>
#include <stdlib.h>

typedef struct {
    double x;
    double y;
    double z;
} Test_Data;

void simulate(Test_Data* data) {
    data->x = 1;
    printf("x = %f\n", data->x);
    data->y = (data->x + 2);
    printf("y = %f\n", data->y);
    data->z = (data->y * 3);
    printf("z = %f\n", data->z);
}

int main() {
    Test_Data data;
    simulate(&data);
    return 0;
}
