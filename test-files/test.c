
int main(int argc, char *argv[]) {
  volatile int x = 10 / 0;
  (void)x;
  return 0;
}
