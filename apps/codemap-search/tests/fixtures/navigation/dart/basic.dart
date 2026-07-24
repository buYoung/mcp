import 'package:demo/helper.dart';

class DartFlow {
  void targetDart() {}
  void callerDart() {
    targetDart();
  }
}
