export function bindRovingTablist(
    root: ParentNode,
    tabSelector: string,
    getTabValue: (tab: HTMLButtonElement) => string,
    onSelect: (value: string) => void,
): void {
    const tabs = Array.from(root.querySelectorAll<HTMLButtonElement>(tabSelector));
    if (tabs.length === 0) return;

    const activate = (target: HTMLButtonElement, moveFocus: boolean) => {
        for (const tab of tabs) {
            const selected = tab === target;
            tab.setAttribute("aria-selected", String(selected));
            tab.tabIndex = selected ? 0 : -1;
        }
        if (moveFocus) {
            target.focus();
            target.scrollIntoView({ block: "nearest", inline: "nearest" });
        }
        onSelect(getTabValue(target));
    };

    for (const tab of tabs) {
        tab.addEventListener("click", () => activate(tab, false));
        tab.addEventListener("keydown", (event) => {
            const idx = tabs.indexOf(tab);
            if (event.key === "ArrowRight") {
                event.preventDefault();
                activate(tabs[(idx + 1) % tabs.length], true);
            } else if (event.key === "ArrowLeft") {
                event.preventDefault();
                activate(tabs[(idx - 1 + tabs.length) % tabs.length], true);
            } else if (event.key === "Home") {
                event.preventDefault();
                activate(tabs[0], true);
            } else if (event.key === "End") {
                event.preventDefault();
                activate(tabs[tabs.length - 1], true);
            }
        });
    }
}
