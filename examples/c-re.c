int main() {
  volatile int *p = (volatile int *)0;
  *p = 10;
  return 0;
}
