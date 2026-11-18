package spooled;

import partial.*;

// The base is named through a wildcard import, which is not resolved to a declaration.
public class SpooledNotifier implements Notifier {
  @Override
  public boolean send(int count) {
    return count > 1;
  }
}
