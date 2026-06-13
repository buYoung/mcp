/** Fixture exercising JavaScript branch-sensitive extraction (TypeScript grammar today). */

/** A cache helper. */
export class Cache {
  /** Read a value. */
  read() {
    const key = "cache-key";
    return key;
  }

  /**
   * @deprecated use read instead
   */
  legacyRead() {
    return "legacy";
  }
}

/** An exported free function. */
export function publicLookup() {
  return 1;
}

function privateLookup() {
  return 0;
}

describe("Cache", () => {
  it("reads", () => {
    new Cache().read();
  });
});
