#define SQUARE(value) ((value) * (value))

int first_square(int value) {
    return SQUARE(value);
}

int second_square(int value) {
    return first_square(value) + 1;
}

int third_square(int value) {
    return second_square(value) + 1;
}
