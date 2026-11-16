import type { DiffLine, PgUiLabels, TomlToken, ToolRender } from "./types";

export function renderTomlLines(target: HTMLElement, lines: string[], tokenize: (line: string) => TomlToken): void {
    target.dataset.copyValue = lines.join("\n");
    target.replaceChildren();
    lines.forEach((ln, idx) => {
        const row = document.createElement("span");
        row.className = "wb-toml-line";

        const num = document.createElement("span");
        num.className = "wb-gutter";
        num.setAttribute("aria-hidden", "true");
        num.textContent = String(idx + 1);
        row.append(num);

        const body = document.createElement("span");
        body.className = "wb-toml-code";

        const tok = tokenize(ln);
        if (tok.kind === "comment") {
            const c = document.createElement("span");
            c.className = "tk-cmt";
            c.textContent = tok.text;
            body.append(c);
        } else if (tok.kind === "section") {
            const s = document.createElement("span");
            s.className = "tk-sec";
            s.textContent = tok.text;
            body.append(s);
        } else if (tok.kind === "pair") {
            const kSpan = document.createElement("span");
            kSpan.className = "tk-key";
            kSpan.textContent = tok.key;

            const opSpan = document.createElement("span");
            opSpan.textContent = " = ";

            const vSpan = document.createElement("span");
            vSpan.className = "tk-val";
            vSpan.textContent = tok.value;

            body.append(kSpan, opSpan, vSpan);
            if (tok.comment) {
                const cm = document.createElement("span");
                cm.className = "tk-cmt";
                cm.textContent = tok.comment;
                body.append(cm);
            }
        } else {
            body.textContent = tok.text;
        }

        row.append(body);
        target.append(row);
    });
}

export interface InspectorRenderContext {
    tools: ToolRender[];
    baselineOutputs: Map<string, ToolRender>;
    changedSet: Set<string>;
    viewMode: "output" | "diff";
    ui: Partial<PgUiLabels>;
    computeDiff: (base: string[], curr: string[]) => DiffLine[];
}

export function renderInspectorStream(panel: HTMLElement, ctx: InspectorRenderContext): void {
    panel.replaceChildren();

    if (ctx.tools.length === 0) {
        const empty = document.createElement("p");
        empty.className = "wb-empty-msg";
        empty.textContent = ctx.ui.noChangedTools ?? "";
        panel.append(empty);
        return;
    }

    const enc = new TextEncoder();
    for (const item of ctx.tools) {
        const base = ctx.baselineOutputs.get(item.id);
        const isChanged = ctx.changedSet.has(item.id);
        const baseLineSet = new Set(base ? base.lines : []);

        const curBytes = enc.encode(item.lines.join("\n")).length;
        const baseBytes = base ? enc.encode(base.lines.join("\n")).length : curBytes;
        const delta = curBytes - baseBytes;

        const card = document.createElement("article");
        card.className = `wb-panel${isChanged ? " is-changed" : ""}`;

        const hdr = document.createElement("header");
        hdr.className = "wb-panel-hdr";

        const leftHdr = document.createElement("div");
        leftHdr.className = "wb-panel-left";

        const toolName = document.createElement("code");
        toolName.className = "wb-tool-name";
        toolName.textContent = item.id;
        leftHdr.append(toolName);

        const paramList = document.createElement("div");
        paramList.className = "wb-params";
        if (item.params.length === 0) {
            const pChip = document.createElement("code");
            pChip.className = "wb-param";
            pChip.textContent = "{}";
            paramList.append(pChip);
        } else {
            for (const [pk, pv] of item.params) {
                const pChip = document.createElement("code");
                pChip.className = "wb-param";
                pChip.textContent = `${pk}=${pv}`;
                paramList.append(pChip);
            }
        }
        leftHdr.append(paramList);

        const rightHdr = document.createElement("div");
        rightHdr.className = "wb-panel-right";

        if (delta !== 0) {
            const deltaEl = document.createElement("span");
            deltaEl.className =
                delta < 0 ? "cm-badge cm-badge--mono cm-badge--positive" : "cm-badge cm-badge--mono cm-badge--negative";
            deltaEl.textContent = delta < 0 ? `${delta} B` : `+${delta} B`;
            rightHdr.append(deltaEl);
        }

        const bytesEl = document.createElement("span");
        bytesEl.className = "wb-bytes";
        bytesEl.textContent = `${curBytes.toLocaleString()} B`;
        rightHdr.append(bytesEl);

        const stateEl = document.createElement("span");
        stateEl.className = isChanged ? "cm-badge cm-badge--accent" : "cm-badge";
        stateEl.textContent = isChanged ? (ctx.ui.modifiedBadge ?? "Modified") : (ctx.ui.unchangedBadge ?? "Default");
        rightHdr.append(stateEl);

        hdr.append(leftHdr, rightHdr);

        const pre = document.createElement("pre");
        pre.className = "wb-code";
        pre.tabIndex = 0;

        const code = document.createElement("code");
        if (ctx.viewMode === "diff" && base && isChanged) {
            const diffRows = ctx.computeDiff(base.lines, item.lines);
            for (const dr of diffRows) {
                const rowEl = document.createElement("span");
                rowEl.className =
                    dr.kind === "del" ? "wb-line is-del" : dr.kind === "add" ? "wb-line is-add" : "wb-line";

                const numEl = document.createElement("span");
                numEl.className = "wb-gutter";
                numEl.setAttribute("aria-hidden", "true");
                numEl.textContent = dr.lineNo;

                const sigEl = document.createElement("span");
                sigEl.className = "wb-sig";
                sigEl.setAttribute("aria-hidden", "true");
                sigEl.textContent = dr.kind === "del" ? "-" : dr.kind === "add" ? "+" : " ";

                const txtEl = document.createElement("span");
                txtEl.className = "wb-txt";
                txtEl.textContent = dr.text || " ";

                rowEl.append(numEl, sigEl, txtEl);
                code.append(rowEl);
            }
        } else {
            item.lines.forEach((ln, idx) => {
                const isLineChanged = isChanged && ln.trim() !== "" && !baseLineSet.has(ln);
                const rowEl = document.createElement("span");
                rowEl.className = isLineChanged ? "wb-line is-mod" : "wb-line";

                const numEl = document.createElement("span");
                numEl.className = "wb-gutter";
                numEl.setAttribute("aria-hidden", "true");
                numEl.textContent = String(idx + 1);

                const txtEl = document.createElement("span");
                txtEl.className = "wb-txt";
                txtEl.textContent = ln || " ";

                rowEl.append(numEl, txtEl);
                code.append(rowEl);
            });
        }

        pre.append(code);
        card.append(hdr, pre);
        panel.append(card);
    }
}
