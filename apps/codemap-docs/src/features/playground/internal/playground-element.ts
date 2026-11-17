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
    resetSetting: (form: HTMLFormElement, key: keyof PgState, defaultState: PgState) => void;
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
        #tomlExpanded = false;
        #selectedCat = "all";
        #filterQuery = "";
        #selectedTab = "all";
        #viewMode: "output" | "diff" = "diff";
        #changedToolsOnly = false;
        #baselineOutputs = new Map<string, ToolRender>();
        #spotlightTimer: number | null = null;

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

        #selectToolTabUI(toolId: string): void {
            this.#selectedTab = toolId;
            for (const tab of Array.from(this.querySelectorAll<HTMLButtonElement>("[data-tool-tab]"))) {
                const active = (tab.dataset.toolTab ?? "") === toolId;
                tab.setAttribute("aria-selected", String(active));
                tab.tabIndex = active ? 0 : -1;
            }
        }

        #spotlightTool(toolId: string): void {
            const panel = this.querySelector<HTMLElement>("[data-outputs-panel]");
            const card = panel?.querySelector<HTMLElement>(`[data-tool-card="${toolId}"]`);
            if (!panel || !card) return;

            for (const prev of Array.from(panel.querySelectorAll(".wb-panel.is-spotlight"))) {
                prev.classList.remove("is-spotlight");
            }
            card.scrollIntoView({ behavior: "smooth", block: "nearest" });
            card.classList.add("is-spotlight");
            if (this.#spotlightTimer) window.clearTimeout(this.#spotlightTimer);
            this.#spotlightTimer = window.setTimeout(() => {
                card.classList.remove("is-spotlight");
            }, 900);
        }

        #wireControls(): void {
            const form = this.querySelector<HTMLFormElement>("[data-pg-form]");

            const handleSettingMutation = (target: EventTarget | null) => {
                this.#clearActivePreset();
                const row = target instanceof Element ? target.closest<HTMLElement>(".wb-row") : null;
                const affected = (row?.dataset.affectedTools ?? "")
                    .split(",")
                    .map((x) => x.trim())
                    .filter(Boolean);

                if (affected.length > 0 && this.#selectedTab !== "all" && !affected.includes(this.#selectedTab)) {
                    this.#selectToolTabUI(affected[0]);
                }

                const changedSet = this.#render();
                const focusTarget = affected.find((id) => changedSet.has(id)) ?? affected[0] ?? null;
                if (focusTarget) {
                    this.#spotlightTool(focusTarget);
                }
            };

            form?.addEventListener("change", (e) => handleSettingMutation(e.target));
            form?.addEventListener("submit", (e) => e.preventDefault());

            for (const resetRowBtn of Array.from(this.querySelectorAll<HTMLButtonElement>("[data-reset-row]"))) {
                resetRowBtn.addEventListener("click", (e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    if (!form) return;
                    const key = resetRowBtn.dataset.resetRow as keyof PgState | undefined;
                    if (!key) return;
                    deps.resetSetting(form, key, deps.defaultState);
                    this.#clearActivePreset();
                    this.#render();
                });
            }

            for (const jumpBtn of Array.from(this.querySelectorAll<HTMLButtonElement>("[data-jump-tool]"))) {
                jumpBtn.addEventListener("click", () => {
                    const toolId = jumpBtn.dataset.jumpTool;
                    if (!toolId) return;
                    this.#selectToolTabUI(toolId);
                    this.#render();
                    this.#spotlightTool(toolId);
                });
            }

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
                    this.#selectedCat = "all";
                    for (const catTab of Array.from(this.querySelectorAll<HTMLButtonElement>("[data-cat-btn]"))) {
                        const isAll = catTab.dataset.catBtn === "all";
                        catTab.setAttribute("aria-selected", String(isAll));
                        catTab.tabIndex = isAll ? 0 : -1;
                    }
                    this.#applySettingFilters();
                    this.#selectToolTabUI("all");
                    const changedSet = this.#render();

                    const settingsPane = this.querySelector<HTMLElement>(".wb-settings");
                    const firstModRow = settingsPane?.querySelector<HTMLElement>(".wb-row.is-modified");
                    if (firstModRow) {
                        firstModRow.scrollIntoView({ behavior: "smooth", block: "center" });
                    } else {
                        settingsPane?.scrollTo({ top: 0, behavior: "smooth" });
                    }

                    const panel = this.querySelector<HTMLElement>("[data-outputs-panel]");
                    panel?.scrollTo({ top: 0, behavior: "smooth" });
                    const firstChanged = Array.from(changedSet)[0];
                    if (firstChanged) this.#spotlightTool(firstChanged);
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
                this.#changedToolsOnly = false;
                this.querySelector("[data-changed-only]")?.setAttribute("aria-pressed", "false");
                this.#selectToolTabUI("all");
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

            const expandBtn = this.querySelector<HTMLButtonElement>("[data-toml-expand]");
            const tomlDock = this.querySelector<HTMLElement>("[data-toml-dock]");
            expandBtn?.addEventListener("click", () => {
                this.#tomlExpanded = !this.#tomlExpanded;
                expandBtn.setAttribute("aria-pressed", String(this.#tomlExpanded));
                expandBtn.textContent = this.#tomlExpanded
                    ? (this.#ui.collapseToml ?? "Compact")
                    : (this.#ui.expandToml ?? "Expand");
                tomlDock?.classList.toggle("is-expanded", this.#tomlExpanded);
            });

            const viewBtns = Array.from(this.querySelectorAll<HTMLButtonElement>("[data-view-mode]"));
            for (const btn of viewBtns) {
                btn.addEventListener("click", () => {
                    this.#viewMode = (btn.dataset.viewMode as "output" | "diff") ?? "diff";
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

        #render(): Set<string> {
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
            const byteDeltas = new Map<string, number>();
            const enc = new TextEncoder();

            for (const item of currentOutputs) {
                const base = this.#baselineOutputs.get(item.id);
                if (!base) continue;
                const curBytes = enc.encode(item.lines.join("\n")).length;
                const baseBytes = enc.encode(base.lines.join("\n")).length;
                byteDeltas.set(item.id, curBytes - baseBytes);

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
                if (tid === "all") continue;
                const isMod = changedSet.has(tid);
                tab.classList.toggle("has-diff", isMod);
                const d = byteDeltas.get(tid) ?? 0;
                tab.title = isMod && d !== 0 ? `${tid} (${d < 0 ? d : `+${d}`} B)` : "";
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
            if (!panel) return changedSet;

            const visibleTools = currentOutputs.filter((item) => {
                if (this.#selectedTab !== "all") return item.id === this.#selectedTab;
                if (this.#changedToolsOnly) return changedSet.has(item.id);
                return true;
            });

            if (this.#selectedTab === "all" && changedSet.size > 0 && !this.#changedToolsOnly) {
                visibleTools.sort((a, b) => Number(changedSet.has(b.id)) - Number(changedSet.has(a.id)));
            }

            deps.renderStream(panel, {
                tools: visibleTools,
                baselineOutputs: this.#baselineOutputs,
                changedSet,
                byteDeltas,
                viewMode: this.#viewMode,
                ui: this.#ui,
                computeDiff: deps.computeDiff,
            });

            return changedSet;
        }
    }

    customElements.define("codemap-config-playground", CodemapConfigPlayground);
}
