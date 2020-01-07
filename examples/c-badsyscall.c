#include <stdio.h>

#include <unistd.h>
#include <sys/fcntl.h>

int main() {
  printf("%d", fcntl(STDIN_FILENO, F_GETFD));
  return 0;
}
