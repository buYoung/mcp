/** Fixture exercising JSX (TSX grammar today) branch-sensitive extraction. */

/** A button component owner. */
export class Button {
  /** Render the button. */
  render() {
    const label = "click me";
    return <button>{label}</button>;
  }

  /**
   * @deprecated use render instead
   */
  legacyRender() {
    return <button>legacy</button>;
  }
}

/** An exported helper. */
export function makeButton() {
  return new Button();
}

function privateButtonHelper() {
  return 0;
}

describe("Button", () => {
  it("renders", () => {
    new Button().render();
  });
});
