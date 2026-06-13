/** Fixture exercising TSX (TypeScript + JSX) branch-sensitive extraction. */

/** A panel component owner. */
export class Panel {
  /** Render the panel. */
  render(): JSX.Element {
    const title = "panel title";
    return <div className="panel">{title}</div>;
  }

  /**
   * @deprecated use render instead
   */
  legacyRender(): JSX.Element {
    return <span>legacy</span>;
  }
}

/** An exported helper. */
export function makePanel(): Panel {
  return new Panel();
}

function privatePanelHelper(): number {
  return 0;
}

describe("Panel", () => {
  it("renders", () => {
    new Panel().render();
  });
});
