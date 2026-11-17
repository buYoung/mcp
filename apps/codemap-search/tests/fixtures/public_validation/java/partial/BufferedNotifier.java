package partial;

import java.util.ArrayList;
import java.util.List;

// A generic type is not linked to the declarations it implements.
public class BufferedNotifier<T> implements Notifier {
  private final List<T> pending = new ArrayList<>();

  @Override
  public boolean send(int count) {
    return pending.size() >= count;
  }
}
