import type { PgState } from "./types";

const PII_KEYS = ["EMAIL_ADDRESS", "IP_ADDRESS", "KR_RRN", "CREDIT_CARD"] as const;
const JEV_SCOPES = ["search", "read", "grep"] as const;

export function readPlaygroundForm(form: HTMLFormElement): PgState {
    const fd = new FormData(form);
    const piiEntities: string[] = [];
    for (const ent of PII_KEYS) {
        if (fd.has(`pii_${ent}`)) piiEntities.push(ent);
    }

    const jevScope: string[] = [];
    for (const sc of JEV_SCOPES) {
        if (fd.has(`jev_${sc}`)) jevScope.push(sc);
    }

    return {
        contextEnabled: fd.has("contextEnabled"),
        navPrecise: fd.has("navPrecise"),
        callerLimit: Number(fd.get("callerLimit") ?? 1000),
        includeTests: fd.has("includeTests"),
        eventsEnabled: fd.has("eventsEnabled"),
        detailFileLimit: Number(fd.get("detailFileLimit") ?? 24),
        snippetMaxLines: Number(fd.get("snippetMaxLines") ?? 500),
        outsideCallers: fd.has("outsideCallers"),
        grepMaxColumns: Number(fd.get("grepMaxColumns") ?? 0),
        includeSeen: fd.has("includeSeen"),
        redactEnabled: fd.has("redactEnabled"),
        piiEntities,
        sensWebhook: fd.has("sensWebhook"),
        customRule: fd.has("customRule"),
        jevEnabled: fd.has("jevEnabled"),
        jevScope,
        jevModel: String(fd.get("jevModel") ?? "jev-1.14.0"),
        indexMode: (fd.get("indexMode") as PgState["indexMode"]) ?? "auto",
        storeRefs: fd.has("storeRefs"),
        exclDirs: (fd.get("exclDirs") as PgState["exclDirs"]) ?? "default",
        docSupport: fd.has("docSupport"),
        fsPerm: (fd.get("fsPerm") as PgState["fsPerm"]) ?? "anywhere",
        synonymsEnabled: fd.has("synonymsEnabled"),
        globalInst: fd.has("globalInst"),
    };
}

export function writePlaygroundForm(form: HTMLFormElement, state: PgState): void {
    const setCheck = (name: string, val: boolean) => {
        const el = form.elements.namedItem(name) as HTMLInputElement | null;
        if (el) el.checked = val;
    };
    const setRadio = (name: string, val: string) => {
        const list = form.querySelectorAll<HTMLInputElement>(`input[name="${name}"]`);
        for (const r of list) r.checked = r.value === val;
    };

    setCheck("contextEnabled", state.contextEnabled);
    setCheck("navPrecise", state.navPrecise);
    setRadio("callerLimit", String(state.callerLimit));
    setCheck("includeTests", state.includeTests);
    setCheck("eventsEnabled", state.eventsEnabled);
    setRadio("detailFileLimit", String(state.detailFileLimit));
    setRadio("snippetMaxLines", String(state.snippetMaxLines));
    setCheck("outsideCallers", state.outsideCallers);
    setRadio("grepMaxColumns", String(state.grepMaxColumns));
    setCheck("includeSeen", state.includeSeen);
    setCheck("redactEnabled", state.redactEnabled);
    for (const ent of PII_KEYS) {
        setCheck(`pii_${ent}`, state.piiEntities.includes(ent));
    }
    setCheck("sensWebhook", state.sensWebhook);
    setCheck("customRule", state.customRule);
    setCheck("jevEnabled", state.jevEnabled);
    for (const sc of JEV_SCOPES) {
        setCheck(`jev_${sc}`, state.jevScope.includes(sc));
    }
    const modelSel = form.elements.namedItem("jevModel") as HTMLSelectElement | null;
    if (modelSel) modelSel.value = state.jevModel;
    setRadio("indexMode", state.indexMode);
    setCheck("storeRefs", state.storeRefs);
    setRadio("exclDirs", state.exclDirs);
    setCheck("docSupport", state.docSupport);
    setRadio("fsPerm", state.fsPerm);
    setCheck("synonymsEnabled", state.synonymsEnabled);
    setCheck("globalInst", state.globalInst);
}
