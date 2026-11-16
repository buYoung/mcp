import type { PgLocale, PgState, TomlToken } from "./types";

export function buildTomlConfig(
    s: PgState,
    defaultState: PgState,
    locale: PgLocale,
    mode: "diff" | "full",
): { lines: string[]; modifiedKeys: Set<string> } {
    const full = mode === "full";
    const sections: Array<[string, string[]]> = [];
    const mod = new Set<string>();

    const push = (sec: string, lns: string[]) => {
        if (lns.length > 0) sections.push([sec, lns]);
    };

    if (s.globalInst !== defaultState.globalInst) mod.add("globalInst");
    if (full || s.globalInst !== defaultState.globalInst) {
        const note = locale === "ko" ? " # 전역 ~/.codemap/config.toml 전용" : " # global ~/.codemap/config.toml only";
        push("[global_instructions]", [`enabled = ${s.globalInst}${note}`]);
    }

    if (s.redactEnabled !== defaultState.redactEnabled) mod.add("redactEnabled");
    if (full || s.redactEnabled !== defaultState.redactEnabled) {
        push("[output]", [`is_redact_enabled = ${s.redactEnabled}`]);
    }

    const searchLines: string[] = [];
    if (s.detailFileLimit !== defaultState.detailFileLimit) mod.add("detailFileLimit");
    if (full || s.detailFileLimit !== defaultState.detailFileLimit) {
        searchLines.push(`detail_file_limit = ${s.detailFileLimit}`);
    }
    if (s.snippetMaxLines !== defaultState.snippetMaxLines) mod.add("snippetMaxLines");
    if (full || s.snippetMaxLines !== defaultState.snippetMaxLines) {
        searchLines.push(`snippet_max_lines = ${s.snippetMaxLines}`);
    }
    push("[output.search]", searchLines);

    if (s.outsideCallers !== defaultState.outsideCallers) mod.add("outsideCallers");
    if (full || s.outsideCallers !== defaultState.outsideCallers) {
        push("[output.read]", [`is_outside_callers_enabled = ${s.outsideCallers}`]);
    }

    if (s.grepMaxColumns !== defaultState.grepMaxColumns) mod.add("grepMaxColumns");
    if (full || s.grepMaxColumns !== defaultState.grepMaxColumns) {
        push("[output.grep]", [`max_columns = ${s.grepMaxColumns}`]);
    }

    const ctxLines: string[] = [];
    if (s.contextEnabled !== defaultState.contextEnabled) mod.add("contextEnabled");
    if (full || s.contextEnabled !== defaultState.contextEnabled) {
        ctxLines.push(`is_enabled = ${s.contextEnabled}`);
    }
    if (s.callerLimit !== defaultState.callerLimit) mod.add("callerLimit");
    if (full || s.callerLimit !== defaultState.callerLimit) {
        ctxLines.push(`caller_limit = ${s.callerLimit}`);
    }
    push("[output.context]", ctxLines);

    if (s.includeTests !== defaultState.includeTests) mod.add("includeTests");
    if (full || s.includeTests !== defaultState.includeTests) {
        push("[output.context.exclude]", [`should_include_test_code = ${s.includeTests}`]);
    }

    if (s.navPrecise !== defaultState.navPrecise) mod.add("navPrecise");
    if (full || s.navPrecise !== defaultState.navPrecise) {
        push("[output.navigation]", [`is_enabled = ${s.navPrecise}`]);
    }

    if (s.eventsEnabled !== defaultState.eventsEnabled) mod.add("eventsEnabled");
    if (full || s.eventsEnabled !== defaultState.eventsEnabled) {
        push("[output.event_navigation]", [`is_enabled = ${s.eventsEnabled}`]);
    }

    const redactLines: string[] = [];
    if (s.piiEntities.length > 0) mod.add("piiEntities");
    if (full || s.piiEntities.length > 0) {
        redactLines.push(`pii_entities = [${s.piiEntities.map((x) => `"${x}"`).join(", ")}]`);
    }
    if (s.sensWebhook) mod.add("sensWebhook");
    if (full || s.sensWebhook) {
        redactLines.push(`sensitive_fields = [${s.sensWebhook ? '"webhookSigningKey"' : ""}]`);
    }
    if (s.customRule) mod.add("customRule");
    if (full || s.customRule) {
        redactLines.push(
            s.customRule
                ? `rules = [{ id = "custom.service-token", pattern = 'glsa_(?P<secret>[A-Za-z0-9_]{16,})' }]`
                : "rules = []",
        );
    }
    push("[output.redact]", redactLines);

    const jevLines: string[] = [];
    const defaultScopeStr = defaultState.jevScope.join(",");
    const curScopeStr = s.jevScope.join(",");
    if (s.jevEnabled !== defaultState.jevEnabled) mod.add("jevEnabled");
    if (curScopeStr !== defaultScopeStr) mod.add("jevScope");
    if (s.jevModel !== defaultState.jevModel) mod.add("jevModel");
    if (full || s.jevEnabled || curScopeStr !== defaultScopeStr || s.jevModel !== defaultState.jevModel) {
        jevLines.push(`enabled = ${s.jevEnabled}`);
        jevLines.push(`scope = [${s.jevScope.map((x) => `"${x}"`).join(", ")}]`);
        jevLines.push(`model = "${s.jevModel}"`);
    }
    push("[output.jev]", jevLines);

    const idxLines: string[] = [];
    if (s.indexMode !== defaultState.indexMode) mod.add("indexMode");
    if (full || s.indexMode !== defaultState.indexMode) {
        idxLines.push(`mode = "${s.indexMode}"`);
    }
    if (s.storeRefs !== defaultState.storeRefs) mod.add("storeRefs");
    if (full || s.storeRefs !== defaultState.storeRefs) {
        idxLines.push(`store_references = ${s.storeRefs}`);
    }
    push("[index]", idxLines);

    if (s.exclDirs !== defaultState.exclDirs) mod.add("exclDirs");
    if (full || s.exclDirs !== defaultState.exclDirs) {
        push("[index.exclude]", [
            s.exclDirs === "empty"
                ? "excluded_directories = []"
                : 'excluded_directories = [".idea", ".vscode", ".vs", "node_modules", ".yarn", "target", "dist", "build", "vendor"]',
        ]);
    }

    if (s.docSupport !== defaultState.docSupport) mod.add("docSupport");
    if (full || s.docSupport !== defaultState.docSupport) {
        push("[index.language_support]", [`is_document_support_enabled = ${s.docSupport}`]);
    }

    if (s.fsPerm !== defaultState.fsPerm) mod.add("fsPerm");
    if (full || s.fsPerm !== defaultState.fsPerm) {
        const permBlock = [`find = "${s.fsPerm}"`, `grep = "${s.fsPerm}"`, `read = "${s.fsPerm}"`];
        if (s.fsPerm === "allowed_roots") {
            permBlock.push('allowed_roots = ["../grafana-plugins"]');
        }
        push("[filesystem_permissions]", permBlock);
    }

    if (s.synonymsEnabled !== defaultState.synonymsEnabled) mod.add("synonymsEnabled");
    if (full || s.synonymsEnabled !== defaultState.synonymsEnabled) {
        push("[related_terms]", [`enabled = ${s.synonymsEnabled}`]);
    }

    if (s.includeSeen !== defaultState.includeSeen) mod.add("includeSeen");

    if (sections.length === 0) {
        const comment =
            locale === "ko"
                ? "# 모든 설정이 기본값입니다. 바꿀 키만 .codemap/config.toml에 적으세요."
                : "# Using built-in defaults. Add only keys you want to override.";
        return { lines: [comment], modifiedKeys: mod };
    }

    const outLines: string[] = [];
    sections.forEach(([hdr, lns], idx) => {
        if (idx > 0) outLines.push("");
        outLines.push(hdr);
        outLines.push(...lns);
    });
    return { lines: outLines, modifiedKeys: mod };
}

export function tokenizeTomlLine(line: string): TomlToken {
    if (line.startsWith("#")) {
        return { kind: "comment", text: line };
    }
    if (line.startsWith("[")) {
        return { kind: "section", text: line };
    }
    const eqIdx = line.indexOf(" = ");
    if (eqIdx >= 0) {
        const rest = line.slice(eqIdx + 3);
        const cmtIdx = rest.indexOf(" #");
        return {
            kind: "pair",
            key: line.slice(0, eqIdx),
            value: cmtIdx >= 0 ? rest.slice(0, cmtIdx) : rest,
            comment: cmtIdx >= 0 ? rest.slice(cmtIdx) : "",
        };
    }
    return { kind: "plain", text: line || " " };
}
