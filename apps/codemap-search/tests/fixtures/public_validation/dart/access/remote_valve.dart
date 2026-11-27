import 'valve.dart';

class RemoteValve extends Valve {
  // Valve._seal is private to its library, so this declares another method.
  void _seal(String reason) {}

  @override
  void vent(String reason) {}
}
