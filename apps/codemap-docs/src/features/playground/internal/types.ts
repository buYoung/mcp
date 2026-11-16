import type { ChipOption, SegmentOption } from "@shared/ui";

export type PgLocale = "en" | "ko";

export interface PgState {
    contextEnabled: boolean;
    navPrecise: boolean;
    callerLimit: number;
    includeTests: boolean;
    eventsEnabled: boolean;
    detailFileLimit: number;
    snippetMaxLines: number;
    outsideCallers: boolean;
    grepMaxColumns: number;
    includeSeen: boolean;
    redactEnabled: boolean;
    piiEntities: string[];
    sensWebhook: boolean;
    customRule: boolean;
    jevEnabled: boolean;
    jevScope: string[];
    jevModel: string;
    indexMode: "auto" | "on" | "off";
    storeRefs: boolean;
    exclDirs: "default" | "empty";
    docSupport: boolean;
    fsPerm: "anywhere" | "workspace" | "allowed_roots";
    synonymsEnabled: boolean;
    globalInst: boolean;
}

export interface ToolRender {
    id: string;
    params: Array<[string, string]>;
    lines: string[];
}

export interface DiffLine {
    kind: "same" | "del" | "add";
    text: string;
    lineNo: string;
}

export type TomlToken =
    | { kind: "comment"; text: string }
    | { kind: "section"; text: string }
    | { kind: "pair"; key: string; value: string; comment: string }
    | { kind: "plain"; text: string };

export type SettingControlSpec =
    | { kind: "switch"; id: string; name: string; checked?: boolean }
    | { kind: "segment"; labelId: string; name: string; mono?: boolean; options: SegmentOption[] }
    | { kind: "chips"; labelId: string; options: ChipOption[] }
    | { kind: "select"; id: string; name: string; options: SegmentOption[] };

export interface SettingItemSpec {
    settingKey: keyof PgState;
    keywords: string;
    title: string;
    tomlKey: string;
    stacked?: boolean;
    control: SettingControlSpec;
}

export interface SettingGroupSpec {
    id: string;
    title: string;
    items: SettingItemSpec[];
}

export interface PresetSpec {
    id: string;
    label: string;
    active?: boolean;
}

export interface PgUiLabels {
    docsLink: string;
    docsHref: string;
    presetDefault: string;
    presetPrecise: string;
    presetJev: string;
    presetSecurity: string;
    presetBroad: string;
    resetBtn: string;
    copyToml: string;
    copyShort: string;
    copiedMsg: string;
    copyFailedMsg: string;
    defaultBadge: string;
    statusFmt: string;
    catAll: string;
    catContext: string;
    catLimits: string;
    catRedact: string;
    catJev: string;
    catIndex: string;
    filterPlaceholder: string;
    tomlChangedOnly: string;
    tomlFull: string;
    groupContext: string;
    groupLimits: string;
    groupRedact: string;
    groupJev: string;
    groupIndex: string;
    lblContextEnabled: string;
    lblNavPrecise: string;
    lblCallerLimit: string;
    lblIncludeTests: string;
    lblEvents: string;
    lblDetailFiles: string;
    lblSnippetLines: string;
    lblOutsideCallers: string;
    lblGrepCols: string;
    lblIncludeSeen: string;
    lblRedactMaster: string;
    lblPiiEntities: string;
    lblSensField: string;
    lblCustomRule: string;
    lblJevEnabled: string;
    lblJevScope: string;
    lblJevModel: string;
    lblIndexMode: string;
    lblStoreRefs: string;
    lblExcludedDirs: string;
    optDirsDefault: string;
    optDirsEmpty: string;
    lblDocSupport: string;
    lblPerms: string;
    lblSynonyms: string;
    lblGlobalInst: string;
    outLabel: string;
    tabAll: string;
    modeOutput: string;
    modeDiff: string;
    changedOnly: string;
    unchangedBadge: string;
    modifiedBadge: string;
    noChangedTools: string;
    noFilterMatch: string;
}
