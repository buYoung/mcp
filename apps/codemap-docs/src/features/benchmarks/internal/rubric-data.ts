import type { RubricEntry, RubricQuestion, RubricTermId } from "./rubric-types";

export { RUBRIC_QUESTION_IDS } from "./rubric-types";
export type {
    InlineSegment,
    RubricEntry,
    RubricFactItem,
    RubricLang,
    RubricQuestion,
    RubricQuestionId,
    RubricTermId,
} from "./rubric-types";

const KO_CORE_QUESTIONS: [RubricQuestion, RubricQuestion, RubricQuestion] = [
    {
        id: "git-sync-job-dispatch",
        tabLabel: "Git 동기화 작업",
        singleTitle: "git-sync-job-dispatch · Core (7)",
        items: [
            {
                lang: "TS ↔ Go",
                text: "TS 화면 버튼과 Go 리포지토리 컨트롤러·웹훅의 동기화 요청이 버스 메시지가 아니라 Go `JobQueue.Insert`로 모입니다.",
            },
            {
                lang: "Go",
                text: "`JobQueue.Insert`가 메모리 큐 대신 typed client `Create`로 API 서버에 `Job` 리소스를 저장합니다.",
                hasSideEffect: true,
            },
            {
                lang: "Go",
                text: "KV 백엔드 쓰기 커밋 후 `publishWatchNotification`이 `us.watch.v1.{group}.{namespace}.{resource}` subject로 NATS 알림을 발행합니다.",
                hasSideEffect: true,
            },
            {
                lang: "Go",
                text: "NATS 활성 시 job informer가 같은 규칙의 와일드카드 subject를 `provisioning-informer` 큐 그룹으로 구독합니다(비활성 시 apiserver `SharedInformer`).",
            },
            {
                lang: "Go",
                text: "알림이 최소 `namespace/name` `Job` 객체로 `OnAdd`에 전달되고, 드라이버 `AddFunc`가 해당 키를 workqueue에 넣습니다.",
            },
            {
                lang: "Go",
                text: "워커가 API `Update`로 `claim` 라벨을 써 작업을 선점하고, `IsSupported`로 `pull` 처리기(`SyncWorker`)를 선택합니다.",
                hasSideEffect: true,
            },
            {
                lang: "Go",
                text: "버스는 본문 없는 메타데이터 신호만 나르며(작업 내용은 API 리소스로 전달), 알림이 빠지거나 NATS가 꺼져도 주기 `re-list`가 작업을 재투입합니다.",
            },
        ],
    },
    {
        id: "alertmanager-config-autosync",
        tabLabel: "Alertmanager 동기화",
        singleTitle: "alertmanager-config-autosync · Core (7)",
        items: [
            {
                lang: "TS",
                text: "설정 화면이 k8s 클라이언트로 싱글턴 `Config` `default`를 읽고, 저장·해제 시 `/spec/externalAlertmanagerSync`를 JSON-patch합니다.",
                hasSideEffect: true,
            },
            {
                lang: "Go",
                text: "어드미션 검증기가 `Config` 쓰기(`default` 이름)를 검사하고, 주입된 콜백으로 ini 우선 설정 차단과 Mimir/Cortex 데이터소스 여부를 확인합니다.",
            },
            {
                lang: "Go",
                text: "동기화는 `Config` 컨트롤러가 아니라 `MultiOrgAlertmanager` 주기 폴링 선처리의 org별 `FetchExtraConfig`로 실행됩니다.",
            },
            {
                lang: "Go",
                text: "대상 UID는 ini `external_alertmanager_uid`를 우선 사용하고, 없으면 워커가 서비스 신원 k8s 클라이언트로 `Config` `default`를 읽습니다.",
            },
            {
                lang: "Go",
                text: "외부 설정을 데이터소스 프록시 HTTP로 조회한 뒤 `SaveAndApplyExtraConfiguration`으로 Alertmanager 설정 저장소에 저장합니다.",
                hasSideEffect: true,
            },
            {
                lang: "Go",
                text: "워커가 동기화 결과를 `Config.status`(`ExternalAlertmanagerSynced` 조건과 `status.externalAlertmanagerSync`)에 기록하고, 없으면 싱글턴을 생성합니다.",
                hasSideEffect: true,
            },
            {
                lang: "TS ↔ Go",
                text: "TS 화면이 Go 워커의 조건 문자열(`ExternalAlertmanagerSynced`, `MergeCommitted`, `NotConfigured`)로 동기화·대기·ini 관리 상태를 해석합니다.",
            },
        ],
    },
    {
        id: "scope-annotation-filter",
        tabLabel: "스코프 주석",
        singleTitle: "scope-annotation-filter · Core (7)",
        items: [
            {
                lang: "TS",
                text: "`scopeFilters`가 켜지면 `ScopesContext`·`ScopesVariable`이 노출되며, 주석 경로에는 스코프 `metadata.name` 문자열 배열만 전달됩니다.",
            },
            {
                lang: "TS",
                text: "Grafana 데이터소스 `getAnnotations`가 `request.scopes`를 `annotationServer().query`로 넘기고, 패널 컨텍스트는 현재 스코프 이름을 `save`·`update`에 전달합니다.",
            },
            {
                lang: "TS",
                text: "주석 목록 패널이 `ScopesContext`를 구독해 스코프 이름이 바뀔 때마다 `annotationServer()`를 거치지 않고 `annotationK8sClient.search`를 직접 호출합니다.",
            },
            {
                lang: "TS",
                text: "`annotationServer()`는 플래그와 `annotation.grafana.app` discovery가 모두 참일 때만 k8s 클라이언트를 쓰며, 아니면 레거시 `/api/annotations`가 `scopes`를 버립니다.",
                hasSideEffect: true,
            },
            {
                lang: "TS",
                text: "k8s 주석 클라이언트가 `/apis/annotation.grafana.app/v0alpha1/.../search`에 반복 `scope` 쿼리 키를 보내고, 생성·수정 요청 본문에 `spec.scopes`를 담아 전송합니다.",
            },
            {
                lang: "Go",
                text: "`[annotations.app_platform] enabled`가 참일 때만 annotation 앱이 설치되며, 커스텀 `search` 핸들러가 `scope`를 `ListOptions.Scopes`로 파싱해 `store.List`를 호출합니다.",
            },
            {
                lang: "Go",
                text: "`store_backend`로 저장소를 선택하며, `memory`·`postgres`·`gRPC`는 스코프를 저장·필터하지만 기본 `legacy-sql` 어댑터는 저장·조회 모두에서 스코프를 누락합니다.",
                hasSideEffect: true,
            },
        ],
    },
];

const KO_EXTENDED_QUESTIONS: [RubricQuestion, RubricQuestion, RubricQuestion] = [
    {
        id: "git-sync-job-dispatch",
        tabLabel: "Git 동기화 작업",
        singleTitle: "git-sync-job-dispatch · Extended (5)",
        items: [
            {
                lang: "Go",
                text: "구독에 실패해도 job informer가 `re-list`만으로 시작해 백그라운드에서 재구독을 시도하고 재연결 시 다시 `re-list`합니다.",
            },
            {
                lang: "Go",
                text: "같은 job informer·드라이버 조립이 apiserver post-start hook과 독립 job-queue operator 두 호스트에 연결됩니다.",
            },
            {
                lang: "Go",
                text: "`UpdateFunc`가 라이브 `MODIFIED` 알림을 무시해 claim·진행 상태 쓰기로 인한 중복 재큐잉을 막습니다.",
                hasSideEffect: true,
            },
            {
                lang: "Go",
                text: "알림 발행은 best-effort로 동작해 발행이 실패해도 로그·지표만 남기고 커밋된 쓰기는 실패하지 않습니다.",
                hasSideEffect: true,
            },
            {
                lang: "Go",
                text: "claim 이후 실패하면 쿨다운에 들어가고, claim 실패는 `maxClaimAttempts`(`3`회) 재시도 후 큐에서 빼 다음 `re-list`에 맡깁니다.",
                hasSideEffect: true,
            },
        ],
    },
    {
        id: "alertmanager-config-autosync",
        tabLabel: "Alertmanager 동기화",
        singleTitle: "alertmanager-config-autosync · Extended (5)",
        items: [
            {
                lang: "Go",
                text: "`Config` 인가는 사용자의 `create`와 모든 `delete`를 막고, `create`와 `configs/status:update`를 서비스 신원 전용으로 제한합니다.",
            },
            {
                lang: "TS ↔ Go",
                text: "managed route로 병합이 끝나면 Go 워커가 status `True`·reason `MergeCommitted`로 동기화를 종료하고, TS 화면은 이를 별도 상태로 표시합니다.",
            },
            {
                lang: "TS ↔ Go",
                text: "자동 동기화가 설정된 동안 Go convert API가 수동 Alertmanager 설정 import를 HTTP `409`로 차단하고, TS 화면은 import 진입점을 비활성화합니다.",
                hasSideEffect: true,
            },
            {
                lang: "TS ↔ Go",
                text: "Go 서버가 `managed_by`(`auto-sync` 또는 `manual`)를 계산하고, TS 화면은 이 값으로 동기화 관리 분기를 결정합니다.",
            },
            {
                lang: "Go",
                text: "동기화 실패를 `SyncReason`으로 분류해 지표·조건 reason에 기록하며, Mimir 평문 `404`(설정 없음)와 프록시 JSON `404`(실패)를 구분합니다.",
            },
        ],
    },
    {
        id: "scope-annotation-filter",
        tabLabel: "스코프 주석",
        singleTitle: "scope-annotation-filter · Extended (5)",
        items: [
            {
                lang: "Go",
                text: "`max_scope_count`(기본 `5`)가 생성·수정 시 `validateScopeCount`로 이어져 초과하면 HTTP `400`을 반환합니다.",
            },
            {
                lang: "TS",
                text: "k8s 조회에서도 알림 상태 주석은 레거시(`type=alert`)에서 따로 가져와 병합하며, 이때 `scopes`와 `scopesMatchAny`를 제거합니다.",
                hasSideEffect: true,
            },
            {
                lang: "Go",
                text: "CLI 플래그 기본값은 `memory`지만 Grafana ini 기본값은 `legacy-sql`이어서 기본 구성에서는 스코프 필터가 무시됩니다.",
            },
            {
                lang: "TS ↔ Go",
                text: "비테스트 TS 호출부에서 `scopesMatchAny`를 채우는 곳이 없어 Go 검색 핸들러의 기본값(`any` 매칭)만 쓰입니다.",
            },
            {
                lang: "Go",
                text: "Graphite 이벤트 핸들러가 k8s 어댑터의 `Create`를 재사용해 같은 검증 경로를 탑니다.",
            },
        ],
    },
];

const EN_CORE_QUESTIONS: [RubricQuestion, RubricQuestion, RubricQuestion] = [
    {
        id: "git-sync-job-dispatch",
        tabLabel: "Git sync jobs",
        singleTitle: "git-sync-job-dispatch · Core (7)",
        items: [
            {
                lang: "TS ↔ Go",
                text: "Sync requests from the TS UI button, Go repository controller, or Go webhook all enter Go `JobQueue.Insert` rather than publishing directly to a bus.",
            },
            {
                lang: "Go",
                text: "`JobQueue.Insert` persists a `Job` resource on the API server via typed client `Create` instead of an in-memory queue.",
                hasSideEffect: true,
            },
            {
                lang: "Go",
                text: "After KV backend commit, `publishWatchNotification` publishes a NATS watch notification on `us.watch.v1.{group}.{namespace}.{resource}`.",
                hasSideEffect: true,
            },
            {
                lang: "Go",
                text: "When NATS is enabled, the job informer subscribes to the matching wildcard subject in queue group `provisioning-informer` (else apiserver `SharedInformer`).",
            },
            {
                lang: "Go",
                text: "Notifications become minimal `namespace/name` `Job` objects for `OnAdd`, and the driver's `AddFunc` enqueues the key into the workqueue.",
            },
            {
                lang: "Go",
                text: "The worker claims the job by writing the `claim` label via API `Update`, then selects `SyncWorker` for `pull` via `IsSupported`.",
                hasSideEffect: true,
            },
            {
                lang: "Go",
                text: "The bus carries metadata-only signals (payload stays in API storage); periodic `re-list` re-enqueues jobs if notifications drop or NATS is off.",
            },
        ],
    },
    {
        id: "alertmanager-config-autosync",
        tabLabel: "Alertmanager sync",
        singleTitle: "alertmanager-config-autosync · Core (7)",
        items: [
            {
                lang: "TS",
                text: "Settings UI reads singleton `Config` `default` via the k8s client and saves or disables by JSON-patching `/spec/externalAlertmanagerSync`.",
                hasSideEffect: true,
            },
            {
                lang: "Go",
                text: "Admission validation checks `Config` writes (`default` name) and runs the injected datasource callback (rejecting ini-managed setups and non-Mimir/Cortex types).",
            },
            {
                lang: "Go",
                text: "Sync runs in `MultiOrgAlertmanager` periodic polling (`FetchExtraConfig` per org), not a `Config` controller.",
            },
            {
                lang: "Go",
                text: "Target UID reads ini `external_alertmanager_uid` first; otherwise the worker reads `Config` `default` with a service-identity k8s client.",
            },
            {
                lang: "Go",
                text: "External config is fetched over datasource proxy HTTP and saved to the Alertmanager config store via `SaveAndApplyExtraConfiguration`.",
                hasSideEffect: true,
            },
            {
                lang: "Go",
                text: "The worker writes sync results to `Config.status` (`ExternalAlertmanagerSynced` and `status.externalAlertmanagerSync`), creating the singleton if absent.",
                hasSideEffect: true,
            },
            {
                lang: "TS ↔ Go",
                text: "The TS UI matches Go worker condition literals (`ExternalAlertmanagerSynced`, `MergeCommitted`, `NotConfigured`) to render synced, pending, or ini-managed state.",
            },
        ],
    },
    {
        id: "scope-annotation-filter",
        tabLabel: "Scope annotations",
        singleTitle: "scope-annotation-filter · Core (7)",
        items: [
            {
                lang: "TS",
                text: "When `scopeFilters` is enabled, `ScopesContext` and `ScopesVariable` expose selected scopes, passing only `metadata.name` strings to annotation code.",
            },
            {
                lang: "TS",
                text: "Datasource `getAnnotations` forwards `request.scopes` to `annotationServer().query`, while panel context passes current scope names to `save`/`update`.",
            },
            {
                lang: "TS",
                text: "The annotation list panel subscribes to `ScopesContext` and calls `annotationK8sClient.search` directly on scope changes, bypassing `annotationServer()`.",
            },
            {
                lang: "TS",
                text: "`annotationServer()` uses the k8s client only when the flag and `annotation.grafana.app` discovery pass; otherwise legacy `/api/annotations` drops `scopes`.",
                hasSideEffect: true,
            },
            {
                lang: "TS",
                text: "The k8s client sends repeated `scope` query params to `/apis/annotation.grafana.app/v0alpha1/.../search` and `spec.scopes` on create/patch.",
            },
            {
                lang: "Go",
                text: "The annotation app installs only when `[annotations.app_platform] enabled` is true; its custom `search` handler maps `scope` to `ListOptions.Scopes` for `store.List`.",
            },
            {
                lang: "Go",
                text: "`store_backend` selects the store: `memory`, `postgres`, and `gRPC` persist and filter scopes, while default `legacy-sql` drops them on writes and queries.",
                hasSideEffect: true,
            },
        ],
    },
];

const EN_EXTENDED_QUESTIONS: [RubricQuestion, RubricQuestion, RubricQuestion] = [
    {
        id: "git-sync-job-dispatch",
        tabLabel: "Git sync jobs",
        singleTitle: "git-sync-job-dispatch · Extended (5)",
        items: [
            {
                lang: "Go",
                text: "Job informer supports degraded start: if subscription fails, it runs on `re-list` alone and retries subscribing in the background, re-listing on reconnect.",
            },
            {
                lang: "Go",
                text: "The same job informer and driver assembly is wired in both the API server post-start hook and the standalone job-queue operator.",
            },
            {
                lang: "Go",
                text: "`UpdateFunc` ignores live `MODIFIED` notifications so claim and progress writes do not re-enqueue the job.",
                hasSideEffect: true,
            },
            {
                lang: "Go",
                text: "Watch notification publishing is best-effort: publish failures record logs and metrics without failing the committed write.",
                hasSideEffect: true,
            },
            {
                lang: "Go",
                text: "Post-claim failures enter cooldown, and claim failures retry up to `maxClaimAttempts` (`3`) before dropping from the queue until `re-list`.",
                hasSideEffect: true,
            },
        ],
    },
    {
        id: "alertmanager-config-autosync",
        tabLabel: "Alertmanager sync",
        singleTitle: "alertmanager-config-autosync · Extended (5)",
        items: [
            {
                lang: "Go",
                text: "`Config` authorization blocks human `create` and all `delete` calls, restricting `create` and `configs/status:update` to the service identity.",
            },
            {
                lang: "TS ↔ Go",
                text: "Once merged into managed routes, the Go worker stops syncing with status `True` and reason `MergeCommitted`, and the TS UI renders it as a distinct state.",
            },
            {
                lang: "TS ↔ Go",
                text: "While auto-sync is configured, the Go convert API blocks manual Alertmanager config imports with HTTP `409` and the TS UI disables the import entrypoint.",
                hasSideEffect: true,
            },
            {
                lang: "TS ↔ Go",
                text: "The Go server computes `managed_by` (`auto-sync` vs `manual`) and the TS UI branches sync management controls on it.",
            },
            {
                lang: "Go",
                text: "Sync failures map to `SyncReason` for metrics and status, separating Mimir plain-text `404` (no config) from proxy JSON `404` (error).",
            },
        ],
    },
    {
        id: "scope-annotation-filter",
        tabLabel: "Scope annotations",
        singleTitle: "scope-annotation-filter · Extended (5)",
        items: [
            {
                lang: "Go",
                text: "`max_scope_count` (default `5`) wires into `validateScopeCount` on create and update, returning HTTP `400` when exceeded.",
            },
            {
                lang: "TS",
                text: "Alert-state annotations are fetched separately from legacy (`type=alert`) and merged with `scopes` and `scopesMatchAny` stripped.",
                hasSideEffect: true,
            },
            {
                lang: "Go",
                text: "The CLI flag defaults to `memory` while Grafana's ini defaults to `legacy-sql`, so scope filtering is ignored under default config.",
            },
            {
                lang: "TS ↔ Go",
                text: "No non-test TS caller sets `scopesMatchAny`, so the Go search handler always falls back to its default (`any` match).",
            },
            {
                lang: "Go",
                text: "The Graphite event handler reuses the k8s adapter's `Create`, sharing the same validation path.",
            },
        ],
    },
];

export function getRubricEntries(locale: "en" | "ko"): Record<RubricTermId, RubricEntry> {
    if (locale === "ko") {
        return {
            core: {
                id: "core",
                popoverId: "bench-rubric-core",
                titleId: "bench-rubric-core-title",
                label: "Core",
                chipLabel: "Core 7",
                title: "Core · 문항당 7개",
                criteria:
                    "답변이 빠뜨릴 수 없는 필수 주 경로·`TS ↔ Go` 계약·전제 교정 항목으로, 코드 근거 인용까지 모두 충족해야 `Core 완결 (7/7)`입니다.",
                tablistLabel: "벤치마크 문항",
                sideEffectLabel: "side effect",
                questions: KO_CORE_QUESTIONS,
                closeLabel: "닫기",
            },
            extended: {
                id: "extended",
                popoverId: "bench-rubric-extended",
                titleId: "bench-rubric-extended-title",
                label: "Extended",
                chipLabel: "Extended 5",
                title: "Extended · 문항당 5개",
                criteria:
                    "주 경로에서 생략할 수 있는 분기·실패/재시도·인가·설정 예외 항목으로, 코드 근거 인용 기준으로 `Core 완결 (7/7)`과 별도 채점합니다.",
                tablistLabel: "벤치마크 문항",
                sideEffectLabel: "side effect",
                questions: KO_EXTENDED_QUESTIONS,
                closeLabel: "닫기",
            },
        };
    }

    return {
        core: {
            id: "core",
            popoverId: "bench-rubric-core",
            titleId: "bench-rubric-core-title",
            label: "Core",
            chipLabel: "Core 7",
            title: "Core · 7 / question",
            criteria:
                "Required main-path hops, `TS ↔ Go` contracts, and premise checks proven by cited source lines; all `7` must pass for `All-core (7/7)`.",
            tablistLabel: "Benchmark questions",
            sideEffectLabel: "side effect",
            questions: EN_CORE_QUESTIONS,
            closeLabel: "Close",
        },
        extended: {
            id: "extended",
            popoverId: "bench-rubric-extended",
            titleId: "bench-rubric-extended-title",
            label: "Extended",
            chipLabel: "Extended 5",
            title: "Extended · 5 / question",
            criteria:
                "Optional branch, failure/retry, authorization, and config edge cases graded with source citations separately from `All-core (7/7)`.",
            tablistLabel: "Benchmark questions",
            sideEffectLabel: "side effect",
            questions: EN_EXTENDED_QUESTIONS,
            closeLabel: "Close",
        },
    };
}
