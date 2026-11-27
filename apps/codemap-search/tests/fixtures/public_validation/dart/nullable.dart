class Gauge {
  int level() => 1;
}

// The receivers are declared with a nullable type.
int safe(Gauge? gauge) {
  return gauge?.level() ?? 0;
}

int asserted(Gauge? gauge) {
  return gauge!.level();
}
