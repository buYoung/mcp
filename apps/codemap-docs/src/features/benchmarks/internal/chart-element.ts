import type { BenchmarkPayload } from "./types";

class CodemapLineChart extends HTMLElement {
    #abort: AbortController | null = null;
    #hiddenSeries = new Set<string>();
    #focusSeries: string | null = null;
    #activeEnv: { panel: "acc" | "tok"; idx: number } | null = null;

    connectedCallback(): void {
        this.#abort?.abort();
        const controller = new AbortController();
        this.#abort = controller;
        const { signal } = controller;

        const raw = this.dataset.chart;
        if (!raw) return;
        const data = JSON.parse(raw) as BenchmarkPayload;

        const buttons = Array.from(this.querySelectorAll<HTMLButtonElement>("[data-series-btn]"));
        for (const btn of buttons) {
            const id = btn.dataset.seriesBtn ?? "";
            btn.addEventListener(
                "click",
                () => {
                    if (this.#hiddenSeries.has(id)) {
                        this.#hiddenSeries.delete(id);
                    } else if (this.#hiddenSeries.size < data.series.length - 1) {
                        this.#hiddenSeries.add(id);
                    }
                    btn.setAttribute("aria-pressed", String(!this.#hiddenSeries.has(id)));
                    this.#syncVisibility(data);
                },
                { signal },
            );
            btn.addEventListener(
                "pointerenter",
                () => {
                    if (!this.#hiddenSeries.has(id)) {
                        this.#focusSeries = id;
                        this.#syncHighlight();
                    }
                },
                { signal },
            );
            btn.addEventListener(
                "pointerleave",
                () => {
                    this.#focusSeries = null;
                    this.#syncHighlight();
                },
                { signal },
            );
        }

        for (const panelName of ["acc", "tok"] as const) {
            const card = this.querySelector<HTMLElement>(`[data-chart-panel="${panelName}"]`);
            if (!card) continue;
            const bands = Array.from(card.querySelectorAll<HTMLButtonElement>("[data-env-band]"));
            for (const band of bands) {
                const idx = Number(band.dataset.envBand ?? "0");
                const show = () => {
                    this.#activeEnv = { panel: panelName, idx };
                    this.#renderTooltip(data, card, panelName, idx);
                };
                const hide = () => {
                    if (this.#activeEnv?.panel === panelName && this.#activeEnv.idx === idx) {
                        this.#activeEnv = null;
                        this.#clearTooltip(card);
                    }
                };
                band.addEventListener("pointerenter", show, { signal });
                band.addEventListener("focus", show, { signal });
                band.addEventListener("pointerleave", hide, { signal });
                band.addEventListener("blur", hide, { signal });
                band.addEventListener(
                    "keydown",
                    (event: KeyboardEvent) => {
                        if (event.key === "Escape") {
                            hide();
                        } else if (event.key === "ArrowRight" && idx < bands.length - 1) {
                            event.preventDefault();
                            bands[idx + 1]?.focus();
                        } else if (event.key === "ArrowLeft" && idx > 0) {
                            event.preventDefault();
                            bands[idx - 1]?.focus();
                        }
                    },
                    { signal },
                );
            }
        }
    }

    disconnectedCallback(): void {
        this.#abort?.abort();
        this.#abort = null;
    }

    #syncHighlight(): void {
        for (const group of this.querySelectorAll<SVGGElement>(".series-group")) {
            const id = group.dataset.seriesId ?? "";
            const dimmed = this.#focusSeries !== null && this.#focusSeries !== id;
            group.classList.toggle("is-dimmed", dimmed);
            group.classList.toggle("is-focused", this.#focusSeries === id);
        }
    }

    #syncVisibility(data: BenchmarkPayload): void {
        const visible = data.series.filter((s) => !this.#hiddenSeries.has(s.id));
        const maxTokVal = Math.max(...visible.flatMap((s) => s.tok), 500);
        const tokCeil = maxTokVal <= 1500 ? 1500 : maxTokVal <= 2000 ? 2000 : 3500;
        const tokTicks =
            tokCeil === 1500
                ? [0, 500, 1000, 1500]
                : tokCeil === 2000
                  ? [0, 500, 1000, 1500, 2000]
                  : [0, 1000, 2000, 3000, 3500];

        const tokGrid = this.querySelector<SVGGElement>('[data-grid-group="tok"]');
        if (tokGrid) {
            const svgNS = "http://www.w3.org/2000/svg";
            tokGrid.replaceChildren();
            tokTicks.forEach((t, idx) => {
                const y = data.padTop + data.plotH - (t / tokCeil) * data.plotH;
                const g = document.createElementNS(svgNS, "g");
                const line = document.createElementNS(svgNS, "line");
                line.setAttribute("class", idx === 0 ? "axis-line" : "grid-line");
                line.setAttribute("x1", String(data.padLeft));
                line.setAttribute("y1", y.toFixed(1));
                line.setAttribute("x2", String(data.svgW - 32));
                line.setAttribute("y2", y.toFixed(1));
                const txt = document.createElementNS(svgNS, "text");
                txt.setAttribute("class", "axis-label");
                txt.setAttribute("x", String(data.padLeft - 8));
                txt.setAttribute("y", (y + 3.5).toFixed(1));
                txt.setAttribute("text-anchor", "end");
                txt.textContent = t === 0 ? "0" : `${t.toLocaleString("en-US")}k`;
                g.append(line, txt);
                tokGrid.append(g);
            });
        }

        for (const panelName of ["acc", "tok"] as const) {
            const container = this.querySelector<SVGGElement>(`[data-lines-group="${panelName}"]`);
            if (!container) continue;
            for (const s of data.series) {
                const g = container.querySelector<SVGGElement>(`[data-series-id="${s.id}"]`);
                if (!g) continue;
                const isHidden = this.#hiddenSeries.has(s.id);
                g.toggleAttribute("hidden", isHidden);
                if (!isHidden && panelName === "tok") {
                    const poly = g.querySelector<SVGPolylineElement>("polyline");
                    const circles = g.querySelectorAll<SVGCircleElement>("circle");
                    const pts = s.tok
                        .map((v, i) => {
                            const y = data.padTop + data.plotH - (v / tokCeil) * data.plotH;
                            circles[i]?.setAttribute("cy", y.toFixed(1));
                            return `${data.xCoords[i].toFixed(1)},${y.toFixed(1)}`;
                        })
                        .join(" ");
                    poly?.setAttribute("points", pts);
                }
            }
        }

        if (this.#activeEnv) {
            const card = this.querySelector<HTMLElement>(`[data-chart-panel="${this.#activeEnv.panel}"]`);
            if (card) {
                this.#renderTooltip(data, card, this.#activeEnv.panel, this.#activeEnv.idx);
            }
        }
    }

    #renderTooltip(data: BenchmarkPayload, card: HTMLElement, panelName: "acc" | "tok", idx: number): void {
        const line = card.querySelector<SVGLineElement>("[data-hover-line]");
        const tip = card.querySelector<HTMLElement>("[data-tooltip]");
        if (!line || !tip) return;

        const x = data.xCoords[idx];
        line.setAttribute("x1", x.toFixed(1));
        line.setAttribute("x2", x.toFixed(1));
        line.classList.add("is-visible");

        for (const pt of card.querySelectorAll<SVGCircleElement>(".series-point")) {
            pt.classList.toggle("is-active", Number(pt.dataset.envIdx) === idx);
        }

        const visible = data.series
            .filter((s) => !this.#hiddenSeries.has(s.id))
            .sort((a, b) => (panelName === "acc" ? b.acc[idx] - a.acc[idx] : a.tok[idx] - b.tok[idx]));

        tip.replaceChildren();
        const heading = document.createElement("div");
        heading.className = "bench-tip-env";
        heading.textContent = data.envs[idx];
        tip.append(heading);

        for (const s of visible) {
            const row = document.createElement("div");
            row.className = s.primary ? "bench-tip-row is-primary" : "bench-tip-row";
            const dot = document.createElement("span");
            dot.className = "bench-tip-dot";
            dot.style.background = s.color;
            const name = document.createElement("span");
            name.className = "bench-tip-name";
            name.textContent = s.label;
            const val = document.createElement("span");
            val.className = "bench-tip-val";
            val.textContent = panelName === "acc" ? s.accDetail[idx] : s.tokDetail[idx];
            row.append(dot, name, val);
            tip.append(row);
        }

        tip.classList.toggle("align-right", idx === 2);
        tip.classList.toggle("align-center", idx === 1);
        tip.classList.toggle("align-left", idx === 0);
        tip.hidden = false;
    }

    #clearTooltip(card: HTMLElement): void {
        const line = card.querySelector<SVGLineElement>("[data-hover-line]");
        const tip = card.querySelector<HTMLElement>("[data-tooltip]");
        line?.classList.remove("is-visible");
        if (tip) tip.hidden = true;
        for (const pt of card.querySelectorAll<SVGCircleElement>(".series-point")) {
            pt.classList.remove("is-active");
        }
    }
}

export function mountBenchmarkCharts(): void {
    if (!customElements.get("codemap-line-chart")) {
        customElements.define("codemap-line-chart", CodemapLineChart);
    }
}
