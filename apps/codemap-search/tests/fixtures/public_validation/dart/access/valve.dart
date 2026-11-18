class Valve {
  void _seal(String reason) {}

  void vent(String reason) {}
}

class LocalValve extends Valve {
  @override
  void _seal(String reason) {}
}
