static int measure_width(void) { return 3; }

static int measure_side(void) { return 2; }

static int measure_scale(void) { return 5; }

/* The call gives a local variable its first value. */
int width(void) {
    int value = measure_width();
    return value;
}

/* One declaration holds two variables, each with a call. */
int area(void) {
    int left = measure_side(), right = measure_side();
    return left * right;
}

/* A recursive call names the function itself. */
int depth(int level) {
    return level ? depth(level - 1) + 1 : 0;
}

/* The parameter hides the function of the same name. */
int scaled(int (*measure_scale)(void)) {
    int value = measure_scale();
    return value;
}

int unscaled(void) {
    return measure_scale();
}
