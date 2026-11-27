package partial;

public class EmailNotifier implements Notifier {
  @Override
  public boolean send(int count) {
    return count > 0;
  }
}
