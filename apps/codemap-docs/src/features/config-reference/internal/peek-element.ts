import { extractPeekContent } from "./peek-extractor";

interface PeekOptions {
    closeLabel: string;
    jumpLabel: string;
    openPageLabel: string;
    piiTitle: string;
    piiHref: string;
    piiTemplate: HTMLTemplateElement | null;
}

class CodemapConfigPeek extends HTMLElement {
    #abort: AbortController | null = null;
    #activeBtn: HTMLButtonElement | null = null;
    #activePeekRow: HTMLElement | null = null;

    connectedCallback(): void {
        this.#abort?.abort();
        const controller = new AbortController();
        this.#abort = controller;
        const { signal } = controller;

        const container = this.closest(".sl-markdown-content");
        if (!container) return;

        const openHashDetails = () => {
            if (!window.location.hash) return;
            const id = decodeURIComponent(window.location.hash.slice(1));
            const target = container.querySelector<HTMLElement>(`#${CSS.escape(id)}`);
            const details = target?.closest("details");
            if (details) details.open = true;
        };
        openHashDetails();
        window.addEventListener("hashchange", openHashDetails, { signal });

        const opts: PeekOptions = {
            closeLabel: this.dataset.closeLabel ?? "Close (Esc)",
            jumpLabel: this.dataset.jumpLabel ?? "Jump to section",
            openPageLabel: this.dataset.openPageLabel ?? "Open full page",
            piiTitle: this.dataset.piiTitle ?? "PII Redaction",
            piiHref: this.dataset.piiHref ?? "/configuration/pii-redaction/",
            piiTemplate: this.querySelector<HTMLTemplateElement>("[data-pii-template]"),
        };

        const keyTables = Array.from(container.querySelectorAll("table")).filter(
            (tbl) => tbl.querySelectorAll("thead th").length === 5,
        );

        for (const keyTable of keyTables) {
            keyTable.classList.add("cfg-key-table");
            const links = Array.from(keyTable.querySelectorAll<HTMLAnchorElement>("tbody a[href]"));
            for (const link of links) {
                const href = link.getAttribute("href") ?? "";
                const isHash = href.startsWith("#") && href.length > 1;
                const isPiiPage = href.includes("/configuration/pii-redaction");
                if (!isHash && !isPiiPage) continue;

                const labelText = (link.textContent ?? "").trim();
                if (!labelText) continue;

                const btn = document.createElement("button");
                btn.type = "button";
                btn.className = "cm-btn cm-btn--subtle cm-btn--xs codemap-lens-btn";
                btn.setAttribute("aria-expanded", "false");
                btn.dataset.peekHref = href;
                btn.dataset.peekKind = isHash ? "section" : "pii";

                const icon = document.createElement("span");
                icon.className = "codemap-lens-chevron";
                icon.setAttribute("aria-hidden", "true");
                icon.textContent = "▸";

                const textSpan = document.createElement("span");
                textSpan.textContent = labelText;

                btn.append(icon, textSpan);
                link.replaceWith(btn);

                btn.addEventListener(
                    "click",
                    () => {
                        if (this.#activeBtn === btn) {
                            this.#closePeek(false);
                            return;
                        }
                        this.#openPeek(btn, container, opts);
                    },
                    { signal },
                );
            }
        }

        document.addEventListener(
            "keydown",
            (event) => {
                if (event.key === "Escape" && this.#activeBtn) {
                    event.preventDefault();
                    this.#closePeek(true);
                }
            },
            { signal },
        );
    }

    disconnectedCallback(): void {
        this.#abort?.abort();
        this.#abort = null;
    }

    #openPeek(btn: HTMLButtonElement, container: Element, opts: PeekOptions): void {
        this.#closePeek(false);

        const row = btn.closest("tr");
        if (!row) return;

        const href = btn.dataset.peekHref ?? "";
        const kind = btn.dataset.peekKind ?? "section";

        let title = "";
        const bodyFrag = document.createDocumentFragment();

        if (kind === "pii" && opts.piiTemplate) {
            title = opts.piiTitle;
            bodyFrag.append(opts.piiTemplate.content.cloneNode(true));
        } else {
            const extracted = extractPeekContent(container, href);
            if (!extracted) return;
            title = extracted.title;
            bodyFrag.append(extracted.frag);
        }

        const peekTr = document.createElement("tr");
        peekTr.className = "codemap-peek-row";

        const td = document.createElement("td");
        td.colSpan = row.children.length || 5;
        td.className = "codemap-peek-cell";

        const panel = document.createElement("div");
        panel.className = "codemap-peek-panel";

        const bar = document.createElement("div");
        bar.className = "codemap-peek-bar";

        const titleEl = document.createElement("span");
        titleEl.className = "codemap-peek-title";
        titleEl.textContent = title;

        const actions = document.createElement("div");
        actions.className = "codemap-peek-actions";

        const navLink = document.createElement("a");
        navLink.className = "cm-btn cm-btn--secondary cm-btn--xs codemap-peek-link";
        navLink.href = kind === "pii" ? opts.piiHref : href;
        navLink.textContent = kind === "pii" ? opts.openPageLabel : opts.jumpLabel;
        if (kind !== "pii") {
            navLink.addEventListener("click", () => {
                const id = decodeURIComponent(href.slice(1));
                const target = container.querySelector<HTMLElement>(`#${CSS.escape(id)}`);
                const details = target?.closest("details");
                if (details) details.open = true;
                this.#closePeek(false);
            });
        }

        const closeBtn = document.createElement("button");
        closeBtn.type = "button";
        closeBtn.className = "cm-btn cm-btn--secondary cm-btn--xs codemap-peek-close";
        closeBtn.textContent = opts.closeLabel;
        closeBtn.addEventListener("click", () => {
            this.#closePeek(true);
        });

        actions.append(navLink, closeBtn);
        bar.append(titleEl, actions);

        const body = document.createElement("div");
        body.className = "codemap-peek-body cm-scroll-pane";
        body.append(bodyFrag);

        panel.append(bar, body);
        td.append(panel);
        peekTr.append(td);

        row.classList.add("has-open-peek");
        row.after(peekTr);

        btn.setAttribute("aria-expanded", "true");
        this.#activeBtn = btn;
        this.#activePeekRow = peekTr;
    }

    #closePeek(restoreFocus: boolean): void {
        const prevBtn = this.#activeBtn;
        if (prevBtn) {
            prevBtn.setAttribute("aria-expanded", "false");
            prevBtn.closest("tr")?.classList.remove("has-open-peek");
        }
        this.#activePeekRow?.remove();
        this.#activeBtn = null;
        this.#activePeekRow = null;
        if (restoreFocus && prevBtn) {
            prevBtn.focus();
        }
    }
}

export function mountConfigPeek(): void {
    if (!customElements.get("codemap-config-peek")) {
        customElements.define("codemap-config-peek", CodemapConfigPeek);
    }
}
