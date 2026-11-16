import type { PgLocale, PgState, ToolRender } from "./types";

type MaskKind = "secret" | "shortSecret" | "urlPass" | "sensWebhook" | "glsa" | "email" | "ip" | "rrn" | "card";

function maskValue(s: PgState, kind: MaskKind, raw: string): string {
    if (!s.redactEnabled) return raw;
    if (kind === "secret") return "[REDACTED]";
    if (kind === "shortSecret") return "*".repeat(raw.length);
    if (kind === "urlPass") return "********";
    if (kind === "sensWebhook") return s.sensWebhook ? "[REDACTED]" : raw;
    if (kind === "glsa") return s.customRule ? "glsa_[REDACTED]" : raw;
    if (kind === "email") return s.piiEntities.includes("EMAIL_ADDRESS") ? "[REDACTED]" : raw;
    if (kind === "ip") return s.piiEntities.includes("IP_ADDRESS") ? "[REDACTED]" : raw;
    if (kind === "rrn") return s.piiEntities.includes("KR_RRN") ? "[REDACTED]" : raw;
    if (kind === "card") return s.piiEntities.includes("CREDIT_CARD") ? "[REDACTED]" : raw;
    return raw;
}

export function simulateToolOutputs(s: PgState, locale: PgLocale): ToolRender[] {
    const taskPrompt =
        locale === "ko"
            ? "대시보드 저장 시 버전 충돌 오류가 반환되는 경로 확인"
            : "Trace how dashboard save returns a version mismatch error";

    const searchParams: Array<[string, string]> = [
        ["query", '"SaveDashboard mailer"'],
        ["workspace_scope", '"pkg"'],
    ];
    if (s.includeSeen) searchParams.push(["include_seen", "true"]);

    const searchLines: string[] = [];
    if (s.indexMode === "off") {
        searchLines.push(
            'Error: search requires an active symbol index, but [index].mode = "off". Use grep, find, and read instead.',
        );
    } else {
        searchLines.push('_Workspace scope: `pkg`. Pass `workspace_scope: "all"` for repo-wide search._');
        searchLines.push("# codemap-search");
        searchLines.push("");
        searchLines.push("## 1. pkg/services/dashboards/service/dashboard_service.go");
        searchLines.push("- `method DashboardServiceImpl.SaveDashboard` (L142-153)");

        if (s.contextEnabled) {
            searchLines.push(
                s.navPrecise
                    ? "  - _callers (tree-sitter precise; import/source resolution confirmed):_"
                    : "  - _callers (approximate name scan):_",
            );

            const callers: string[] = [
                s.navPrecise
                    ? "    - `pkg/api/dashboard.go:318` (`HTTPServer.PostDashboard`, precise)"
                    : "    - `pkg/api/dashboard.go:318` (`HTTPServer.PostDashboard`)",
                s.navPrecise
                    ? "    - `pkg/services/provisioning/dashboards/file_reader.go:204` (`FileReader.saveDashboard`, precise)"
                    : "    - `pkg/services/provisioning/dashboards/file_reader.go:204` (`FileReader.saveDashboard`)",
            ];
            if (s.storeRefs) {
                callers.push("    - `pkg/registry/apis/dashboards/legacy.go:64` (`SaveDashboardDTO` reference)");
            }
            if (s.includeTests) {
                callers.push(
                    "    - `pkg/services/dashboards/service/dashboard_service_test.go:88` (`TestSaveDashboard`)",
                );
            }
            searchLines.push(...callers.slice(0, s.callerLimit));
            if (callers.length > s.callerLimit) {
                searchLines.push(
                    `    - _(+${callers.length - s.callerLimit} more omitted by output.context.caller_limit=${s.callerLimit})_`,
                );
            }

            searchLines.push(
                s.navPrecise ? "  - _calls (depth 1, tree-sitter precise):_" : "  - _calls (depth 1, approximate):_",
            );
            searchLines.push(
                s.navPrecise
                    ? "    - `DashboardServiceImpl.buildSaveDashboardCommand` — `pkg/services/dashboards/service/dashboard_service.go:96` (`precise`)"
                    : "    - `buildSaveDashboardCommand` — `pkg/services/dashboards/service/dashboard_service.go:96`",
            );
        }

        if (s.eventsEnabled) {
            searchLines.push("  - _events (static bus routes):_");
            searchLines.push(
                "    - publish `events.DashboardSaved` → subscribe `pkg/services/live/features/dashboard.go:91`",
            );
        }

        searchLines.push("");
        searchLines.push("### results");
        const fullSaveLines = [
            "142→func (dr *DashboardServiceImpl) SaveDashboard(ctx context.Context, dto *dashboards.SaveDashboardDTO) (*dashboards.Dashboard, error) {",
            "143→    cmd, err := dr.buildSaveDashboardCommand(ctx, dto)",
            "144→    if errors.Is(err, dashboards.ErrDashboardVersionMismatch) {",
            "145→        return nil, err",
            "146→    }",
            "147→    res, err := dr.store.SaveDashboard(ctx, *cmd)",
            "148→    if err != nil {",
            "149→        return nil, err",
            "150→    }",
            "151→    dr.bus.Publish(ctx, &events.DashboardSaved{UID: res.UID})",
            "152→    return res, nil",
            "153→}",
        ];
        if (s.snippetMaxLines < fullSaveLines.length) {
            searchLines.push(...fullSaveLines.slice(0, s.snippetMaxLines));
            searchLines.push(
                `... [snippet capped at ${s.snippetMaxLines} lines; read pkg/services/dashboards/service/dashboard_service.go L${142 + s.snippetMaxLines}-153]`,
            );
        } else {
            searchLines.push(...fullSaveLines);
        }

        if (s.jevEnabled && s.jevScope.includes("search")) {
            searchLines.push(
                `- _omitted body: L210-226 (method DeleteDashboard) Jev (${s.jevModel}) found no task match in pkg/services/dashboards/service/dashboard_service.go._`,
            );
        } else if (!s.includeSeen) {
            searchLines.push("- `method DashboardServiceImpl.DeleteDashboard` (L210-226): `<--removed duplicated-->`");
        } else {
            searchLines.push(
                "210→func (dr *DashboardServiceImpl) DeleteDashboard(ctx context.Context, dashboardId int64, orgId int64) error {",
                "211→    return dr.store.DeleteDashboard(ctx, &dashboards.DeleteDashboardCommand{ID: dashboardId, OrgID: orgId})",
                "212→}",
            );
        }

        const extraCandidates: Array<{ path: string; detail: string[] }> = [
            {
                path: "pkg/services/notifications/mailer.go",
                detail:
                    s.jevEnabled && s.jevScope.includes("search")
                        ? [
                              "## 2. pkg/services/notifications/mailer.go",
                              `- _omitted body: L38-46 (method Mailer.SendNotification) Jev (${s.jevModel}) found no task match in pkg/services/notifications/mailer.go._`,
                          ]
                        : [
                              "## 2. pkg/services/notifications/mailer.go",
                              "- `method Mailer.SendNotification` (L38-46)",
                              `39→    authHeader := "Bearer ${maskValue(s, "secret", "eyJhbGciOiJIUzI1NiJ9.grafana.sig")}"`,
                              `40→    webhookSigningKey := "${maskValue(s, "sensWebhook", "whsec_live_98a7bc65d4ef3210")}"`,
                              `41→    serviceToken := "${maskValue(s, "glsa", "glsa_8Kp2Lm9Qx4Vz7Rn1Yt6Bc3Df")}"`,
                              `42→    notifyFrom := "${maskValue(s, "email", "alerts@grafana.example.com")}"`,
                          ],
            },
        ];

        if (s.docSupport) {
            extraCandidates.push({
                path: "docs/sources/dashboards/build-dashboards/save-dashboard.md",
                detail: [
                    `## ${extraCandidates.length + 2}. docs/sources/dashboards/build-dashboards/save-dashboard.md`,
                    "- `heading Save a dashboard and resolve version conflicts` (L12-34)",
                ],
            });
        }
        if (s.exclDirs === "empty") {
            extraCandidates.push({
                path: "vendor/github.com/grafana/grafana-plugin-sdk-go/backend/dashboard_save.go",
                detail: [
                    `## ${extraCandidates.length + 2}. vendor/github.com/grafana/grafana-plugin-sdk-go/backend/dashboard_save.go`,
                    "- `func HandleDashboardSave` (L18-41)",
                ],
            });
        }

        const tailRows: string[] = [];
        for (let i = 0; i < extraCandidates.length; i++) {
            const rank = i + 2;
            if (rank <= s.detailFileLimit) {
                searchLines.push("");
                searchLines.push(...extraCandidates[i].detail);
            } else {
                tailRows.push(`- ${rank}. \`${extraCandidates[i].path}\``);
            }
        }
        if (tailRows.length > 0) {
            searchLines.push("");
            searchLines.push(`## Ranked tail (compact; detail_file_limit = ${s.detailFileLimit})`);
            searchLines.push(...tailRows);
        }
    }

    const readParams: Array<[string, string]> = [
        ["file_path", '"pkg/services/notifications/mailer.go"'],
        ["offset", "38"],
        ["limit", "14"],
    ];
    if (s.includeSeen) readParams.push(["include_seen", "true"]);

    const readLines: string[] = [
        `[Permission policy: ${s.fsPerm}${s.fsPerm === "allowed_roots" ? " (workspace + ../grafana-plugins)" : ""}]`,
    ];
    if (!s.includeSeen) {
        readLines.push("38–39: <--removed duplicated-->");
    } else {
        readLines.push("38→func (m *Mailer) SendNotification(ctx context.Context, payload []byte) error {");
        readLines.push(`39→    authHeader := "Bearer ${maskValue(s, "secret", "eyJhbGciOiJIUzI1NiJ9.grafana.sig")}"`);
    }
    readLines.push(
        `40→    adminPassword := "${maskValue(s, "shortSecret", "admin")}"`,
        `41→    dsn := "postgres://grafana:${maskValue(s, "urlPass", "s3cr3tPass!")}@${maskValue(s, "ip", "10.24.8.15")}:5432/grafana"`,
        `42→    webhookSigningKey := "${maskValue(s, "sensWebhook", "whsec_live_98a7bc65d4ef3210")}"`,
        `43→    serviceToken := "${maskValue(s, "glsa", "glsa_8Kp2Lm9Qx4Vz7Rn1Yt6Bc3Df")}"`,
        `44→    fromAddr := "${maskValue(s, "email", "alerts@grafana.example.com")}"`,
        "45→    return m.dispatch(ctx, dsn, authHeader, adminPassword, webhookSigningKey, serviceToken, fromAddr, payload)",
        "46→}",
    );

    if (s.jevEnabled && s.jevScope.includes("read")) {
        readLines.push(
            `- _omitted body: L48-51 (func auditMaskSample) Jev (${s.jevModel}) found no task match in pkg/services/notifications/mailer.go._`,
        );
    } else {
        readLines.push(
            "48→func auditMaskSample() (string, string) {",
            `49→    krRrn := "${maskValue(s, "rrn", "900101-1234567")}"`,
            `50→    testCard := "${maskValue(s, "card", "4532-0151-1283-0366")}"`,
            "51→    return krRrn, testCard",
        );
    }

    if (s.indexMode === "off") {
        readLines.push('[Outside callers unavailable: indexing is disabled ([index].mode = "off").]');
    } else if (s.outsideCallers) {
        readLines.push(
            s.navPrecise
                ? "[Callers from outside this window (tree-sitter precise)]"
                : "[Callers from outside this window (static analysis, approximate)]",
        );
        readLines.push(
            "- SendNotification L38: pkg/services/ngalert/notifier/channels/email.go:164 (EmailNotifier.Notify)",
        );
        if (s.includeTests) {
            readLines.push(
                "- SendNotification L38: pkg/services/notifications/mailer_test.go:72 (TestSendNotification)",
            );
        }
        if (s.eventsEnabled) {
            readLines.push("[Event routes: subscribes notifications.EmailQueued]");
        }
    }

    const grepParams: Array<[string, string]> = [
        ["pattern", '"ErrDashboardVersionMismatch|webhookSigningKey"'],
        ["path", '"pkg"'],
        ["view", '"source_grouped"'],
    ];
    const rawGrepLine1 =
        '48:var ErrDashboardVersionMismatch = errors.New("the dashboard has been changed by someone else")';
    const rawGrepLine2 = `42:    webhookSigningKey := "${maskValue(s, "sensWebhook", "whsec_live_98a7bc65d4ef3210")}" // notify=${maskValue(s, "email", "alerts@grafana.example.com")}`;

    const clampCol = (ln: string) => {
        if (s.grepMaxColumns > 0 && ln.length > s.grepMaxColumns) {
            const lineNo = ln.split(":")[0];
            return `${lineNo}:[Omitted long matching line (${ln.length} > max_columns=${s.grepMaxColumns})]`;
        }
        return ln;
    };

    const grepLines: string[] = [
        `# codemap-search (permission: ${s.fsPerm})`,
        "## 1. pkg/services/dashboards/models.go",
        clampCol(rawGrepLine1),
        "",
        "## 2. pkg/services/notifications/mailer.go",
    ];
    if (s.jevEnabled && s.jevScope.includes("grep")) {
        grepLines.push(`- _omitted match: L42 (Mailer.SendNotification) Jev (${s.jevModel}) found no task match._`);
    } else {
        grepLines.push(clampCol(rawGrepLine2));
    }
    if (s.exclDirs === "empty") {
        grepLines.push("");
        grepLines.push("## 3. vendor/github.com/grafana/sdk/errors.go");
        grepLines.push(clampCol('19:    ErrDashboardVersionMismatch = "version-mismatch"'));
    }

    const findParams: Array<[string, string]> = [
        ["pattern", '"*dashboard*service*"'],
        ["path", '"."'],
    ];
    const findLines: string[] = [
        `# permission: ${s.fsPerm}`,
        "pkg/services/dashboards/service/dashboard_service.go",
        "pkg/services/dashboards/service/dashboard_service_test.go",
    ];
    if (s.exclDirs === "empty") {
        findLines.push("public/build/dashboardServiceChunk.b84f1a.js");
        findLines.push("vendor/github.com/grafana/sdk/dashboard_service_mock.go");
    }

    const initParams: Array<[string, string]> = s.jevEnabled ? [["task", `"${taskPrompt}"`]] : [];
    const initLines: string[] = [];
    if (s.globalInst) {
        initLines.push("[Global instructions: active — navigation reminder synced to client instructions file]");
    }
    if (s.jevEnabled) {
        initLines.push(
            `[Jev task registered (${s.jevModel}; scope: ${s.jevScope.join(", ") || "none"}): "${taskPrompt}"]`,
        );
    }
    if (s.indexMode === "off") {
        initLines.push(
            'Indexing is disabled ([index].mode = "off"). Use find, grep, and read for live source navigation.',
        );
    } else {
        initLines.push(`Index mode: "${s.indexMode}". Selectable workspace_scope values:`);
        initLines.push("- pkg (11,420 files; Go)");
        initLines.push("- public (9,810 files; TypeScript, SCSS)");
        initLines.push("- packages/grafana-ui (1,510 files; TypeScript)");
        if (s.docSupport) {
            initLines.push("- docs (1,840 files; Markdown)");
        }
        if (s.exclDirs === "empty") {
            initLines.push("- vendor (3,640 files; Go — included because excluded_directories = [])");
        }
    }
    if (s.synonymsEnabled) {
        initLines.push("- Optional tool `synonyms` is enabled ([related_terms].enabled = true).");
    }

    const analyzeParams: Array<[string, string]> = [
        ["target", '"index"'],
        ["sort", '"stored"'],
        ["limit", "2"],
    ];
    let analyzeLines: string[];
    if (s.indexMode === "off") {
        analyzeLines = JSON.stringify(
            { target: "index", status: "disabled", reason: '[index].mode = "off"' },
            null,
            2,
        ).split("\n");
    } else {
        const baseFiles = 19757 + (s.docSupport ? 1840 : 0) + (s.exclDirs === "empty" ? 4120 : 0);
        const baseSymbols = 312480 + (s.docSupport ? 14200 : 0) + (s.exclDirs === "empty" ? 58900 : 0);
        const baseBytes =
            143130624 +
            (s.storeRefs ? 19400000 : 0) +
            (s.docSupport ? 6200000 : 0) +
            (s.exclDirs === "empty" ? 31500000 : 0);
        const rows: Array<[string, string, number, number]> = [
            ["pkg/services/sqlstore/migrations/migrations.go", "go", s.storeRefs ? 114820 : 98210, 2140],
            s.docSupport
                ? ["docs/sources/administration/configuration/index.md", "markdown", 91400, 1960]
                : ["pkg/api/api.go", "go", s.storeRefs ? 71200 : 61400, 1180],
        ];
        analyzeLines = JSON.stringify(
            {
                target: "index",
                status: "ok",
                index_mode: s.indexMode,
                store_references: s.storeRefs,
                summary: { files: baseFiles, symbols: baseSymbols, stored_bytes: baseBytes },
                columns: ["path", "language", "stored", "lines"],
                rows,
            },
            null,
            2,
        ).split("\n");
    }

    const synParams: Array<[string, string]> = [["terms", '["dashboard", "quota"]']];
    const synLines = s.synonymsEnabled
        ? [
              '### Dashboard & View Layout ("dashboard")',
              'Direct (3 in code): "board", "panel", "scene"',
              "",
              '### Resource Allocation ("quota")',
              'Direct (4 in code): "allowance", "limit", "rate_limit", "threshold"',
          ]
        : [
              'Tool "synonyms" is disabled by default ([related_terms].enabled = false).',
              "Enable [related_terms].enabled = true in .codemap/config.toml to query code-matched related terms.",
          ];

    return [
        { id: "search", params: searchParams, lines: searchLines },
        { id: "read", params: readParams, lines: readLines },
        { id: "grep", params: grepParams, lines: grepLines },
        { id: "find", params: findParams, lines: findLines },
        { id: "initial_instructions", params: initParams, lines: initLines },
        { id: "analyze", params: analyzeParams, lines: analyzeLines },
        { id: "synonyms", params: synParams, lines: synLines },
    ];
}
