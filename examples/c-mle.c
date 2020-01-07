#include <stdlib.h>

int main() {
  volatile int* p = (volatile int *)malloc(512 * 1024 * 1024); // 512 MB of memory to be allocated
  *p = 10;
  free(p);
  return 0;
}
