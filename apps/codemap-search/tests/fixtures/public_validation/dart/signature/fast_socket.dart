import 'socket.dart';

class FastSocket implements Socket {
  static Future<FastSocket> connect(Uri url) async => FastSocket();

  @override
  void send(String text) {}

  @override
  void route(Uri url) {}
}
