import 'package:demo/helper.dart';

class Worker {
  final String name = 'dart';

  @Deprecated('old')
  void run() {
    Helper.start();
  }
}
