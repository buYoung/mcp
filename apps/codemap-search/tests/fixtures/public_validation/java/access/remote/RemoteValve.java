package valves.remote;

import valves.Valve;

public class RemoteValve extends Valve {
  // Valve.seal is visible only inside its package, so this declares another method.
  public void seal(String reason) {}

  @Override
  public void vent(String reason) {}
}
