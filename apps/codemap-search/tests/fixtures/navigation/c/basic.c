#include "user.h"
#include <stdio.h>

struct User {
    int id;
};

void save(struct User *user) {}

void run(void) {
    struct User user;
    save(&user);
    printf("ok");
}
