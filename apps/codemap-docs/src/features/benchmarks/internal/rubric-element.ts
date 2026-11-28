import { RUBRIC_QUESTION_IDS, type RubricQuestionId, type RubricTermId } from "./rubric-types";

const TERM_TO_POPOVER: Record<RubricTermId, string> = {
    core: "bench-rubric-core",
    extended: "bench-rubric-extended",
};

function matchTerm(text: string | null): RubricTermId | null {
    const trimmed = (text ?? "").trim();
    if (trimmed === "Core") return "core";
    if (trimmed === "Extended") return "extended";
    return null;
}

function matchQuestionSlug(text: string | null | undefined): RubricQuestionId | null {
    const trimmed = (text ?? "").trim();
    for (const slug of RUBRIC_QUESTION_IDS) {
        if (trimmed === slug) return slug;
    }
    return null;
}

function findEnclosingQuestionSlug(el: Element): RubricQuestionId | null {
    let li = el.closest("li");
    while (li) {
        const leadingCode = li.querySelector<HTMLElement>(
            ":scope > strong > code, :scope > p > strong > code, :scope > code",
        );
        const slug = matchQuestionSlug(leadingCode?.textContent);
        if (slug) return slug;
        li = li.parentElement?.closest("li") ?? null;
    }
    return null;
}

function findPrecedingQuestionSlug(container: Element, target: Element): RubricQuestionId | null {
    const headings = Array.from(container.querySelectorAll<HTMLElement>("h2, h3"));
    let nearest: HTMLElement | null = null;
    for (const heading of headings) {
        const rel = heading.compareDocumentPosition(target);
        if (rel & Node.DOCUMENT_POSITION_FOLLOWING) {
            nearest = heading;
        } else {
            break;
        }
    }
    if (!nearest) return null;
    return matchQuestionSlug(nearest.id) ?? matchQuestionSlug(nearest.textContent);
}

function createTriggerButton(
    term: RubricTermId,
    label: string,
    variant: "inline" | "th",
    questionId: RubricQuestionId | null,
): HTMLButtonElement {
    const popoverId = TERM_TO_POPOVER[term];
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = `bench-rubric-trigger bench-rubric-trigger--${variant}`;
    btn.dataset.rubricTrigger = term;
    if (questionId) {
        btn.dataset.rubricQuestion = questionId;
    }
    btn.setAttribute("popovertarget", popoverId);
    btn.setAttribute("aria-controls", popoverId);
    btn.setAttribute("aria-expanded", "false");
    btn.textContent = label;
    return btn;
}

class CodemapBenchRubric extends HTMLElement {
    #abort: AbortController | null = null;
    #activeTrigger = new Map<HTMLElement, HTMLButtonElement>();
    #pendingTrigger = new Map<HTMLElement, HTMLButtonElement>();

    connectedCallback(): void {
        this.#abort?.abort();
        const controller = new AbortController();
        this.#abort = controller;
        const { signal } = controller;

        const container = this.closest(".sl-markdown-content");
        if (!container) return;

        this.#enhanceContentTerms(container);

        const popovers = Array.from(this.querySelectorAll<HTMLElement>("[data-rubric-popover]"));
        const byId = new Map<string, HTMLElement>();
        for (const popover of popovers) {
            if (popover.id) byId.set(popover.id, popover);
            this.#bindPopoverTabs(popover, signal);

            const closeBtn = popover.querySelector<HTMLButtonElement>(".bench-rubric-popover-close");
            closeBtn?.addEventListener(
                "click",
                () => {
                    const prev = this.#activeTrigger.get(popover);
                    if (prev) {
                        queueMicrotask(() => prev.focus());
                    }
                },
                { signal },
            );
            popover.addEventListener(
                "toggle",
                (event) => {
                    const toggleEvent = event as ToggleEvent;
                    if (toggleEvent.newState === "open") {
                        const trigger =
                            this.#pendingTrigger.get(popover) ??
                            (document.activeElement instanceof HTMLButtonElement &&
                            document.activeElement.dataset.rubricTrigger
                                ? document.activeElement
                                : null);
                        this.#pendingTrigger.delete(popover);
                        if (trigger) {
                            this.#applyTriggerScope(popover, trigger);
                            this.#setActiveTrigger(popover, trigger);
                            this.#positionPopover(popover, trigger);
                        }
                    } else {
                        this.#pendingTrigger.delete(popover);
                        const prev = this.#activeTrigger.get(popover);
                        if (prev) {
                            prev.setAttribute("aria-expanded", "false");
                            this.#activeTrigger.delete(popover);
                        }
                    }
                },
                { signal },
            );
        }

        container.addEventListener(
            "click",
            (event) => {
                const target = event.target;
                if (!(target instanceof Element)) return;
                const btn = target.closest<HTMLButtonElement>("[data-rubric-trigger]");
                if (!btn) return;

                const popoverId = btn.getAttribute("popovertarget") ?? "";
                const popover = byId.get(popoverId);
                if (!popover) return;

                const isOpen = popover.matches(":popover-open");
                const currentTrigger = this.#activeTrigger.get(popover);
                if (isOpen && currentTrigger && currentTrigger !== btn) {
                    event.preventDefault();
                    this.#applyTriggerScope(popover, btn);
                    this.#setActiveTrigger(popover, btn);
                    this.#positionPopover(popover, btn);
                    return;
                }

                if (!isOpen) {
                    this.#applyTriggerScope(popover, btn);
                    this.#pendingTrigger.set(popover, btn);
                }
            },
            { signal },
        );

        const syncOpenPositions = () => {
            for (const [popover, trigger] of this.#activeTrigger.entries()) {
                if (popover.matches(":popover-open")) {
                    this.#positionPopover(popover, trigger);
                }
            }
        };

        window.addEventListener("resize", syncOpenPositions, { passive: true, signal });
        window.addEventListener("scroll", syncOpenPositions, { passive: true, capture: true, signal });
    }

    disconnectedCallback(): void {
        this.#abort?.abort();
        this.#abort = null;
        this.#activeTrigger.clear();
        this.#pendingTrigger.clear();
    }

    #applyTriggerScope(popover: HTMLElement, trigger: HTMLButtonElement): void {
        const titleEl = popover.querySelector<HTMLElement>(".bench-rubric-popover-title");
        const defaultTitle = titleEl?.dataset.defaultTitle ?? titleEl?.textContent ?? "";
        const questionId = matchQuestionSlug(trigger.dataset.rubricQuestion);

        if (questionId) {
            popover.dataset.rubricScope = "single";
            const panel = this.#selectQuestion(popover, questionId, false);
            if (titleEl) {
                titleEl.textContent = panel?.dataset.singleTitle ?? defaultTitle;
            }
        } else {
            popover.dataset.rubricScope = "all";
            this.#selectQuestion(popover, RUBRIC_QUESTION_IDS[0], false);
            if (titleEl) {
                titleEl.textContent = defaultTitle;
            }
        }
        popover.scrollTop = 0;
    }

    #selectQuestion(
        popover: HTMLElement,
        questionId: string,
        moveFocus: boolean,
    ): HTMLElement | null {
        const tabs = Array.from(popover.querySelectorAll<HTMLButtonElement>("[data-rubric-tab]"));
        const panels = Array.from(popover.querySelectorAll<HTMLElement>("[data-rubric-panel]"));

        let activeTab: HTMLButtonElement | null = null;
        for (const tab of tabs) {
            const selected = tab.dataset.rubricTab === questionId;
            tab.setAttribute("aria-selected", selected ? "true" : "false");
            tab.tabIndex = selected ? 0 : -1;
            if (selected) activeTab = tab;
        }

        let activePanel: HTMLElement | null = null;
        for (const panel of panels) {
            const match = panel.dataset.rubricPanel === questionId;
            panel.hidden = !match;
            if (match) activePanel = panel;
        }

        if (moveFocus && activeTab) {
            activeTab.focus();
        }

        return activePanel;
    }

    #bindPopoverTabs(popover: HTMLElement, signal: AbortSignal): void {
        const tabs = Array.from(popover.querySelectorAll<HTMLButtonElement>("[data-rubric-tab]"));
        if (tabs.length === 0) return;

        const activateTab = (nextTab: HTMLButtonElement, moveFocus: boolean) => {
            const targetId = nextTab.dataset.rubricTab;
            if (!targetId) return;

            this.#selectQuestion(popover, targetId, moveFocus);
            popover.scrollTop = 0;

            const trigger = this.#activeTrigger.get(popover);
            if (trigger && popover.matches(":popover-open")) {
                this.#positionPopover(popover, trigger);
            }
        };

        for (let idx = 0; idx < tabs.length; idx++) {
            const tab = tabs[idx];
            tab.addEventListener(
                "click",
                () => {
                    activateTab(tab, false);
                },
                { signal },
            );
            tab.addEventListener(
                "keydown",
                (event) => {
                    let nextIdx: number | null = null;
                    if (event.key === "ArrowRight") {
                        nextIdx = (idx + 1) % tabs.length;
                    } else if (event.key === "ArrowLeft") {
                        nextIdx = (idx - 1 + tabs.length) % tabs.length;
                    } else if (event.key === "Home") {
                        nextIdx = 0;
                    } else if (event.key === "End") {
                        nextIdx = tabs.length - 1;
                    }
                    if (nextIdx === null) return;
                    event.preventDefault();
                    activateTab(tabs[nextIdx], true);
                },
                { signal },
            );
        }
    }

    #enhanceContentTerms(container: Element): void {
        const codes = Array.from(container.querySelectorAll<HTMLElement>("code"));
        for (const codeEl of codes) {
            if (codeEl.closest("pre, .expressive-code, .not-content, button, a")) continue;
            const term = matchTerm(codeEl.textContent);
            if (!term) continue;
            const questionId = findEnclosingQuestionSlug(codeEl);
            codeEl.replaceWith(
                createTriggerButton(term, (codeEl.textContent ?? "").trim(), "inline", questionId),
            );
        }

        const headers = Array.from(container.querySelectorAll<HTMLTableCellElement>("thead th"));
        for (const th of headers) {
            if (th.closest(".not-content") || th.querySelector("[data-rubric-trigger]")) continue;
            const term = matchTerm(th.textContent);
            if (!term) continue;
            const label = (th.textContent ?? "").trim();
            const questionId = findPrecedingQuestionSlug(container, th);
            th.replaceChildren(createTriggerButton(term, label, "th", questionId));
        }
    }

    #setActiveTrigger(popover: HTMLElement, trigger: HTMLButtonElement): void {
        const prev = this.#activeTrigger.get(popover);
        if (prev && prev !== trigger) {
            prev.setAttribute("aria-expanded", "false");
        }
        trigger.setAttribute("aria-expanded", "true");
        this.#activeTrigger.set(popover, trigger);
    }

    #positionPopover(popover: HTMLElement, trigger: HTMLButtonElement): void {
        const pad = 12;
        const gap = 8;
        const vw = window.innerWidth;
        const vh = window.innerHeight;
        const tRect = trigger.getBoundingClientRect();
        const pRect = popover.getBoundingClientRect();

        const maxLeft = Math.max(pad, vw - pRect.width - pad);
        const left = Math.min(Math.max(tRect.left, pad), maxLeft);

        const belowTop = tRect.bottom + gap;
        const aboveTop = tRect.top - gap - pRect.height;
        let top = belowTop;
        if (belowTop + pRect.height > vh - pad && aboveTop >= pad) {
            top = aboveTop;
        } else {
            top = Math.min(Math.max(belowTop, pad), Math.max(pad, vh - pRect.height - pad));
        }

        popover.style.left = `${Math.round(left)}px`;
        popover.style.top = `${Math.round(top)}px`;
    }
}

export function mountBenchmarkRubric(): void {
    if (!customElements.get("codemap-bench-rubric")) {
        customElements.define("codemap-bench-rubric", CodemapBenchRubric);
    }
}
