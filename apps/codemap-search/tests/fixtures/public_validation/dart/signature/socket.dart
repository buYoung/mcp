abstract class Socket {
  static Future<Socket> connect(Uri url) => throw UnimplementedError();

  void send(String text);

  void route(Uri url);
}
