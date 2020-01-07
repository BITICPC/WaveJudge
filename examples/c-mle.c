#include <stdlib.h>

int main() {
  volatile int* p = (volatile int *)malloc(512 * 1024 * 1024); // 512 MB of memory to be allocated
  for (int i = 0; i < 512 * 1024 * 1024 / sizeof(int); ++i) {
    p[i] = i;
  }
  free(p);
  return 0;
  return 0;
}
