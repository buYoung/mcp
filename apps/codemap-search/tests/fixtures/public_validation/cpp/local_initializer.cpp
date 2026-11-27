namespace gauge {

int sample() { return 4; }

// The call gives a local variable its first value.
int reading() {
    auto value = sample();
    return value;
}

// A recursive call names the function itself.
int countdown(int from) {
    return from ? countdown(from - 1) : 0;
}

}  // namespace gauge
