#include <stdio.h>

int main() {
  int choice;
  printf("Which girl do you like?\n"
         "1. Sayori\n"
         "2. Natsuki\n"
         "3. Yuri\n"
         "4. Monika\n");
  scanf("%d", &choice);
  switch (choice) {
  case 1:
    puts("You choose Sayori");
    break;
  case 2:
    puts("You choose Natsuki");
    break;
  case 3:
    puts("You choose Yuri");
    break;
  case 4:
    puts("Just Monika");
    break;

  default:
    break;
  }
  return 0;
}