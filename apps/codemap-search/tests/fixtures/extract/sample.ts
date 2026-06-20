/** Fixture exercising TypeScript branch-sensitive extraction. */

/** A user service. */
export class UserService {
  /** Fetch the current user. */
  fetch(): string {
    const endpoint = "https://api.example.com/user";
    return endpoint;
  }

  /**
   * @deprecated use fetch instead
   */
  legacyFetch(): string {
    return "legacy";
  }
}

/** An exported free function. */
export function publicHelper(): number {
  return 1;
}

function privateHelper(): number {
  return 0;
}

describe("UserService", () => {
  it("renders", () => {
    new UserService().fetch();
  });
});
