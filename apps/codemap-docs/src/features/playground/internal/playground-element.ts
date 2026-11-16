import type { CopyFeedbackMessages } from "@shared/ui";
import type { InspectorRenderContext } from "./dom-renderer";
import type { DiffLine, PgLocale, PgState, PgUiLabels, TomlToken, ToolRender } from "./types";

export interface PlaygroundDependencies {
    defaultState: PgState;
    presets: Record<string, Partial<PgState>>;
    bindCopyButtons: (root: HTMLElement, targetSelector: string, messages: CopyFeedbackMessages) => void;
    bindRovingTablist: (
        root: ParentNode,
        tabSelector: string,
        getTabValue: (tab: HTMLButtonElement) => string,
        onSelect: (value: string) => void,
    ) => void;
    readForm: (form: HTMLFormElement) => PgState;
    writeForm: (form: HTMLFormElement, state: PgState) => void;
    buildToml: (
        state: PgState,
        defaultState: PgState,
        locale: PgLocale,
        mode: "diff" | "full",
    ) => { lines: string[]; modifiedKeys: Set<string> };
    tokenizeToml: (line: string) => TomlToken;
    simulateOutputs: (state: PgState, locale: PgLocale) => ToolRender[];
    computeDiff: (base: string[], curr: string[]) => DiffLine[];
    renderToml: (target: HTMLElement, lines: string[], tokenize: (line: string) => TomlToken) => void;
    renderStream: (panel: HTMLElement, ctx: InspectorRenderContext) => void;
}

export function mountConfigPlayground(deps: PlaygroundDependencies): void {
    if (customElements.get("codemap-config-playground")) return;

    class CodemapConfigPlayground extends HTMLElement {
        #locale: PgLocale = "ko";
        #ui: Partial<PgUiLabels> = {};
        #tomlMode: "diff" | "full" = "diff";
        #selectedCat = "all";
        #filterQuery = "";
        #selectedTab = "all";
        #viewMode: "output" | "diff" = "output";
        #changedToolsOnly = false;
        #baselineOutputs = new Map<string, ToolRender>();

        connectedCallback(): void {
            const raw = this.dataset.pg;
            if (!raw) return;
            const parsed = JSON.parse(raw) as { locale: PgLocale; ui: Partial<PgUiLabels> };
            this.#locale = parsed.locale;
            this.#ui = parsed.ui;

            for (const item of deps.simulateOutputs(deps.defaultState, this.#locale)) {
                this.#baselineOutputs.set(item.id, item);
            }

            this.#wireControls();
            deps.bindCopyButtons(this, "[data-toml-out]", {
                copied: this.#ui.copiedMsg ?? "Copied",
                failed: this.#ui.copyFailedMsg ?? "Copy failed",
            });
            this.#render();
        }

        #wireControls(): void {
            const form = this.querySelector<HTMLFormElement>("[data-pg-form]");
            form?.addEventListener("input", () => {
                this.#clearActivePreset();
                this.#render();
            });
            form?.addEventListener("change", () => {
                this.#clearActivePreset();
                this.#render();
            });
            form?.addEventListener("submit", (e) => e.preventDefault());

            const presetBtns = Array.from(this.querySelectorAll<HTMLButtonElement>("[data-preset]"));
            for (const btn of presetBtns) {
                btn.addEventListener("click", () => {
                    if (!form) return;
                    const key = btn.dataset.preset ?? "default";
                    const next: PgState = {
                        ...deps.defaultState,
                        piiEntities: [...deps.defaultState.piiEntities],
                        jevScope: [...deps.defaultState.jevScope],
                        ...(deps.presets[key] ?? {}),
                    };
                    deps.writeForm(form, next);
                    for (const b of presetBtns) b.classList.toggle("is-active", b === btn);
                    this.#render();
                });
            }

            this.querySelector<HTMLButtonElement>("[data-reset]")?.addEventListener("click", () => {
                if (!form) return;
                deps.writeForm(form, {
                    ...deps.defaultState,
                    piiEntities: [],
                    jevScope: ["search", "read", "grep"],
                });
                for (const b of presetBtns) {
                    b.classList.toggle("is-active", b.dataset.preset === "default");
                }
                this.#render();
            });

            deps.bindRovingTablist(
                this,
                "[data-cat-btn]",
                (tab) => tab.dataset.catBtn ?? "all",
                (cat) => {
                    this.#selectedCat = cat;
                    this.#applySettingFilters();
                },
            );

            const searchInput = this.querySelector<HTMLInputElement>("[data-key-filter]");
            searchInput?.addEventListener("input", () => {
                this.#filterQuery = searchInput.value.trim().toLowerCase();
                this.#applySettingFilters();
            });

            const modeBtns = Array.from(this.querySelectorAll<HTMLButtonElement>("[data-toml-mode]"));
            for (const btn of modeBtns) {
                btn.addEventListener("click", () => {
                    this.#tomlMode = (btn.dataset.tomlMode as "diff" | "full") ?? "diff";
                    for (const b of modeBtns) b.setAttribute("aria-pressed", String(b === btn));
                    this.#render();
                });
            }

            const viewBtns = Array.from(this.querySelectorAll<HTMLButtonElement>("[data-view-mode]"));
            for (const btn of viewBtns) {
                btn.addEventListener("click", () => {
                    this.#viewMode = (btn.dataset.viewMode as "output" | "diff") ?? "output";
                    for (const b of viewBtns) b.setAttribute("aria-pressed", String(b === btn));
                    this.#render();
                });
            }

            const changedPill = this.querySelector<HTMLButtonElement>("[data-changed-only]");
            changedPill?.addEventListener("click", () => {
                this.#changedToolsOnly = !this.#changedToolsOnly;
                changedPill.setAttribute("aria-pressed", String(this.#changedToolsOnly));
                this.#render();
            });

            deps.bindRovingTablist(
                this,
                "[data-tool-tab]",
                (tab) => tab.dataset.toolTab ?? "all",
                (toolId) => {
                    this.#selectedTab = toolId;
                    this.#render();
                },
            );
        }

        #applySettingFilters(): void {
            const groups = Array.from(this.querySelectorAll<HTMLElement>("[data-cat-group]"));
            let anyVisible = false;

            for (const grp of groups) {
                const cat = grp.dataset.catGroup ?? "";
                const catMatch = this.#selectedCat === "all" || this.#selectedCat === cat;
                const rows = Array.from(grp.querySelectorAll<HTMLElement>(".wb-row"));
                let groupVisible = 0;

                for (const r of rows) {
                    const hay = `${r.dataset.keywords ?? ""} ${r.textContent ?? ""}`.toLowerCase();
                    const qMatch = !this.#filterQuery || hay.includes(this.#filterQuery);
                    const show = catMatch && qMatch;
                    r.hidden = !show;
                    if (show) groupVisible++;
                }

                grp.hidden = groupVisible === 0;
                if (groupVisible > 0) anyVisible = true;
            }

            const emptyEl = this.querySelector<HTMLElement>("[data-empty-filter]");
            if (emptyEl) emptyEl.hidden = anyVisible;
        }

        #clearActivePreset(): void {
            for (const b of Array.from(this.querySelectorAll<HTMLButtonElement>("[data-preset]"))) {
                b.classList.remove("is-active");
            }
        }

        #render(): void {
            const form = this.querySelector<HTMLFormElement>("[data-pg-form]");
            const state = form ? deps.readForm(form) : { ...deps.defaultState };

            const { lines: tomlLines, modifiedKeys } = deps.buildToml(
                state,
                deps.defaultState,
                this.#locale,
                this.#tomlMode,
            );
            const tomlOut = this.querySelector<HTMLElement>("[data-toml-out]");
            if (tomlOut) deps.renderToml(tomlOut, tomlLines, deps.tokenizeToml);

            for (const row of Array.from(this.querySelectorAll<HTMLElement>(".wb-row[data-setting]"))) {
                const k = row.dataset.setting ?? "";
                row.classList.toggle("is-modified", modifiedKeys.has(k));
            }

            const currentOutputs = deps.simulateOutputs(state, this.#locale);
            const changedSet = new Set<string>();

            for (const item of currentOutputs) {
                const base = this.#baselineOutputs.get(item.id);
                if (!base) continue;
                const pChanged = JSON.stringify(base.params) !== JSON.stringify(item.params);
                if (pChanged || base.lines.join("\n") !== item.lines.join("\n")) {
                    changedSet.add(item.id);
                }
            }

            const telemetry = this.querySelector<HTMLElement>("[data-telemetry]");
            if (telemetry) {
                if (modifiedKeys.size === 0) {
                    telemetry.textContent = this.#ui.defaultBadge ?? "All defaults";
                    telemetry.classList.remove("is-active");
                } else {
                    telemetry.textContent = (this.#ui.statusFmt ?? "{m} modified · {t}/7 tools")
                        .replace("{m}", String(modifiedKeys.size))
                        .replace("{t}", String(changedSet.size));
                    telemetry.classList.add("is-active");
                }
            }

            for (const tab of Array.from(this.querySelectorAll<HTMLButtonElement>("[data-tool-tab]"))) {
                const tid = tab.dataset.toolTab ?? "";
                if (tid !== "all") tab.classList.toggle("has-diff", changedSet.has(tid));
            }
            const allLabel = this.querySelector<HTMLElement>("[data-all-tab-label]");
            if (allLabel) {
                allLabel.textContent =
                    changedSet.size > 0
                        ? this.#locale === "ko"
                            ? `전체 (${changedSet.size}/7)`
                            : `All (${changedSet.size}/7)`
                        : (this.#ui.tabAll ?? "All 7");
            }

            const panel = this.querySelector<HTMLElement>("[data-outputs-panel]");
            if (!panel) return;

            const visibleTools = currentOutputs.filter((item) => {
                if (this.#selectedTab !== "all") return item.id === this.#selectedTab;
                if (this.#changedToolsOnly) return changedSet.has(item.id);
                return true;
            });

            deps.renderStream(panel, {
                tools: visibleTools,
                baselineOutputs: this.#baselineOutputs,
                changedSet,
                viewMode: this.#viewMode,
                ui: this.#ui,
                computeDiff: deps.computeDiff,
            });
        }
    }

    customElements.define("codemap-config-playground", CodemapConfigPlayground);
}
