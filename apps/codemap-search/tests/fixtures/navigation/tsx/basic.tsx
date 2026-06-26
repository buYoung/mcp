import { Panel } from "./panel";

export function View() {
    const panel = new Panel();
    return (
        <button type="button" onClick={() => panel.open()}>
            {Panel.title()}
        </button>
    );
}
