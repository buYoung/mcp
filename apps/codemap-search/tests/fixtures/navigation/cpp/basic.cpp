#include "user.hpp"
#include <vector>

class User {
public:
    void save() {}
};

void free_function() {}

void run() {
    User user;
    User *pointer = &user;
    user.save();
    pointer->save();
    free_function();
}
