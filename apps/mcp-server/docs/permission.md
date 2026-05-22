# ACP 코드 에이전트 권한 정책 (MCP-as-ACP-Client, Feasible Subset)

> **본 문서는 "MCP 서버가 ACP 클라이언트로 동작하여 ACP 에이전트(Claude Code / Gemini CLI 등)를
> subprocess로 호출하는" 토폴로지에서 **결정론적 코드 로직만으로 강제 가능한 범위**를 다룬다.**
>
> 원문 정책 중 LLM 협조 없이는 강제 불가능하거나, ACP 프로토콜 가시범위 밖이거나,
> 런타임(Node.js 등) 제약으로 안전하게 구현 불가능한 항목은 **삭제하거나 약화**했고
> 각 항목에 사유를 명시한다.

---

## 0. 신뢰 모델과 가시범위 (NEW)

```yaml
trust_model:
  enforcer: "MCP server (ACP client side)의 결정론적 policy engine"
  assumption: "ACP agent는 cooperative하게 session/request_permission을 송신한다."
  non_assumption:
    - "에이전트가 모든 위험 동작을 빠짐없이 request_permission으로 보고한다고 가정하지 않는다."
    - "에이전트의 LLM 내부 추론/메모리/프롬프트는 정책 엔진의 가시범위 밖이다."

visibility_scope:
  visible_to_policy_engine:
    - "ACP session/request_permission 메시지의 toolCall payload"
    - "ACP fs/read_text_file, fs/write_text_file 요청 (capability advertise 시)"
    - "ACP terminal/create 요청과 그 argv (capability advertise 시)"
    - "에이전트 spawn 시점의 환경변수, cwd, argv"
    - "session/set_mode 요청"

  not_visible_or_partial:
    - "에이전트가 자체 HTTP 클라이언트로 보내는 outbound 요청"
    - "에이전트가 fs capability를 거치지 않고 직접 호출하는 file syscall"
    - "에이전트 LLM context 내부에 로드된 데이터의 가공(embedding/summarization)"
    - "이미 spawn된 에이전트의 환경변수 변경"
    - "에이전트가 session/request_permission을 송신하지 않고 자체 승인한 도구 호출"
    - "current_mode_update notification으로 에이전트가 사후 통보하는 mode 변경"

  non_cooperative_agent_handling:
    rule: >
      "에이전트가 ACP 권한 게이트를 우회하는 것은 프로토콜 위반이며,
       MCP 정책 엔진은 이를 코드 로직만으로 완전히 막을 수 없다.
       이 경우의 방어는 (a) 에이전트 spawn 시 OS 수준 sandbox 적용,
       (b) 외부 firewall/process containment에 위임한다."
    examples:
      - "Linux: Landlock, seccomp, network namespaces"
      - "macOS: sandbox-exec / seatbelt"
      - "containerization: Docker/Podman with --network=none --read-only"
```

---

## 1. ACP 세션 설정 모델 (원문 유지)

```yaml
acp_session_configuration:
  primary_interface:
    type: "session_config_options"
    config_id: "permission_profile"
    category: "mode"
    values: ["read-only", "edit", "full"]
    default: "edit"

  compatibility_interface:
    type: "session_modes"
    enabled: true
    sync_rule: "configOptions.permission_profile == modes.currentModeId"

  runtime_change_policy:
    on_set_mode_request: "session/set_mode 또는 session/set_config_option 수신 시"
    allow:
      - "full → edit"
      - "full → read-only"
      - "edit → read-only"
    deny_in_same_session:
      - "read-only → edit"
      - "read-only → full"
      - "edit → full"
    deny_response: { kind: "reject", message: "권한 상승은 현재 세션에서 허용되지 않습니다." }

  # NEW (사유: ACP는 에이전트 자율 mode 변경을 current_mode_update notification으로 허용)
  agent_autonomous_mode_change:
    handling: >
      "에이전트가 current_mode_update notification으로 mode 상승을 통보하더라도,
       MCP 정책 엔진은 자신의 내부 mode 상태를 변경하지 않는다.
       즉, 에이전트의 자칭 mode와 정책 엔진의 enforce mode가 분리된다.
       모든 후속 request_permission 판정은 엔진의 enforce mode를 기준으로 한다."
    audit: "에이전트의 자율 mode 변경 시도를 모두 audit log에 기록한다."
```

---

## 2. 권한 요청 처리 (원문 유지)

```yaml
permission_request_policy:
  method: "session/request_permission"

  supported_options:
    - { kind: "allow_once",    scope: "이번 작업만 허용" }
    - { kind: "allow_always",  scope: "동일 policy scope 내 지속 허용" }
    - { kind: "reject_once",   scope: "이번 작업만 거부" }
    - { kind: "reject_always", scope: "동일 policy scope 내 지속 거부" }

  timeout:
    default_decision: "reject_once"

  layer_0_override: false

  # NEW 한계 명시
  limitations:
    - "옵션은 allow/reject 양자택일. 명령어 rewrite(예: --ignore-scripts 자동 삽입)는 불가."
    - "에이전트가 request_permission을 보내지 않으면 정책 엔진은 그 동작을 알 수 없다."
```

---

## 3. Layer 0 — Hard Block (가시범위 내에서 강제)

> 각 항목에 **가시범위 표시**를 추가했다.
> `[visible]` = ACP 메시지로 항상 가시
> `[partial]` = ACP 메시지 경유 시에만 가시
> `[external]` = OS 샌드박스 등 외부 메커니즘이 필요

### 3.1 파괴적 파일시스템

```yaml
destructive_fs:
  hard_block_via_terminal_tool: # [partial: terminal tool 경유 시]
    - "rm -rf /"
    - "rm -rf ~"
    - "rm -rf $HOME"
    - "find ... -delete"
    - "find ... -exec rm ..."
    - "dd if=... of=/dev/..."
    - "mkfs / fdisk / diskutil eraseDisk"
    - "shred / wipe / srm"
    - "rsync --delete with target outside CWD"

  hard_block_via_fs_capability: # [partial: ACP fs/write_text_file 경유 시]
    - "CWD 외부 경로 write/delete"
    - "민감 경로(~/.ssh, ~/.aws 등) write"

  defense_in_depth:
    - "에이전트 spawn 시 OS 샌드박스(read-only mount, Landlock 등)로 CWD 외 쓰기 차단" # [external]
```

### 3.2 쉘 우회 / 코드 인젝션

```yaml
shell_evasion:
  hard_block_via_terminal_tool: # [partial]
    - "sh -c / bash -c / zsh -c with untrusted string"
    - "eval"
    - "xargs with destructive command"
    - "$(...) / backticks 안의 destructive 명령"
    - "base64 / python -c / perl -e / ruby -e / node -e one-liner로 FS·network mutation"
  classification:
    rule: "shell wrapper 또는 인터프리터 one-liner는 고위험으로 재분류한다."
  limitation: "에이전트가 자체 spawn API(child_process.spawn 등)를 직접 호출하면 보이지 않는다."
```

### 3.3 파괴적 Git (원문 유지, terminal tool 경유 가시)

```yaml
destructive_git: # [partial: terminal tool 경유 시]
  hard_block:
    - "git push --force / --force-with-lease (protected branch)"
    - "git push --mirror"
    - "git push --delete protected branch"
    - "git push origin :protected-branch"
    - "git reset --hard (protected branch)"
    - "git update-ref -d"
    - "git reflog expire"
    - "git gc --prune=now"
    - "git filter-branch / git filter-repo"
    - "git rebase (protected branch)"
    - "git commit --amend (이미 push된 브랜치)"
    - "git 명령 내 --no-verify"
    - ".git/hooks/** write"
    - ".husky/** write"

  move_to_confirm_gate:
    - "git checkout ."
    - "git restore ."
    - "git branch -D <non-protected local branch>"
    - "git clean -fdx (CWD 내부)"
    - "git stash drop / clear"

  protected_branch_resolution:
    sources:
      - "git config: ACP-mcp.protectedBranches"
      - "default: main, master, release/*, prod/*, production"
      - "remote default branch (origin/HEAD)"
```

### 3.4 시크릿 접근 (약화)

```yaml
secret_access:
  blocked_read: # [partial: ACP fs/read_text_file 또는 terminal tool 경유 시]
    credential_dirs:
      - "~/.ssh/**"
      - "~/.aws/**"
      - "~/.gnupg/**"
      - "~/.docker/config.json"
      - "~/.kube/**"
      - "~/.config/gh/**"
      - "~/.config/gcloud/**"
      - "~/.azure/**"
      - "~/.npmrc"
      - "~/.pypirc"
      - "~/.netrc"
    project_secrets:
      - "**/.env"
      - "**/.env.local"
      - "**/.env.production"
      - "**/.env.staging"
      - "**/.env.*.local"
      - "**/*.pem"
      - "**/*.key"
      - "**/*id_rsa*"
      - "**/*id_ed25519*"
      - "**/service-account*.json"
      - "**/*credentials*.json"
      - "**/*.tfvars"
      - "**/secrets.yaml"
      - "**/secrets.yml"

  allow_read_templates:
    - ".env.example"
    - ".env.sample"
    - ".env.template"
    - ".env.defaults"

  content_heuristic: # [partial: ACP fs/read_text_file 응답을 정책 엔진이 가로채는 경우만]
    rule: >
      "허용 템플릿이라도 파일 내용 스캔 결과
       (API_KEY|TOKEN|SECRET|PASSWORD|PRIVATE_KEY|ACCESS_KEY) 패턴의
       non-empty value 가 있으면 deny."
    limitation: "에이전트가 fs capability를 거치지 않으면 적용 불가."

  # === 원문 대비 약화 ===
  denied_direct_exposure: # 강제 가능
    - "ACP fs/read_text_file로 secret 파일 read"
    - "terminal tool로 cat/grep/less 등 secret 파일 read"
    - "terminal tool로 secret 파일 copy/move/upload"

  best_effort_indirect_exposure: # 약화: best-effort only
    - "에이전트 응답 스트림에서 known secret 패턴 redaction (정규식 기반)"
    - "audit log에 secret 값 미저장"
  removed_from_hard_block: # 사유 명시
    - item: "embedding / indexing"
      reason: "LLM 내부 처리는 ACP 가시범위 밖이라 코드로 강제 불가."
    - item: "summarization"
      reason: "동상. read 시점에 차단하는 것이 유일한 방어."
    - item: "diff / grep 결과 노출 차단"
      reason: "에이전트 응답 스트림 후처리만 가능. 완전성 보장 불가."

  allowed_metadata_only:
    - "존재 여부 확인"
    - "파일명 리스팅 (값 redaction 포함)"
```

### 3.5 시스템 변경 (원문 유지)

```yaml
system_modification: # [partial: terminal tool 경유 시]
  hard_block:
    - "sudo / su / doas"
    - "chown"
    - "OS package manager: apt, yum, dnf, pacman, brew install"
    - "글로벌 패키지 설치: npm i -g, pnpm add -g, yarn global add"
    - "사용자/전역 tool 설치: pip install --user, pipx install, uv tool install, cargo install, go install pkg@version"

  chmod_policy:
    rule: "결과 permission bit 평가 기반."
    parse: "symbolic mode 및 octal mode 모두 파싱."
    hard_block_if_result:
      - "world-writable 비트 설정"
      - "민감/설정 파일에 world-executable"
      - "재귀적 권한 확대 (-R + world-writable)"
    confirm_gate:
      - "신규 스크립트에 chmod +x"
      - "CWD 내 재귀 chmod"
    limitation: "에이전트가 fs.chmod()를 직접 호출하면 보이지 않음."
```

### 3.6 자격증명 변경 (원문 유지)

```yaml
credential_changes: # [partial: terminal tool 경유 시]
  hard_block:
    - "git config --global"
    - "SSH key 생성/삭제"
    - "자격증명/토큰 파일 write"
    - "gh auth login/logout"
    - "aws configure"
    - "gcloud auth"
    - "docker login"
```

### 3.7 외부 유출 경로 (한계 명시)

```yaml
exfiltration_paths:
  hard_block_via_terminal_tool: # [partial]
    - "unknown external host로의 outbound"
    - "curl/wget POST/PUT/PATCH/DELETE with body or @file"
    - "curl --data / --data-binary / --form"
    - "wget --post-data / --post-file"
    - "scp"
    - "rsync 원격 전송"
    - "nc / ncat raw socket"
    - "ssh user@host '...' (원격 명령/stdin 포워딩)"
    - "DNS exfiltration: dig/nslookup with file-derived subdomain"
    - "git clone/fetch/push https://x:$TOKEN@host/repo"

  limitation: >
    "에이전트가 자체 HTTP 클라이언트(fetch, axios 등)로 보내는 요청은 보이지 않는다.
     이 영역은 OS 샌드박스 / 네트워크 namespace / 외부 egress proxy로 막아야 한다." # [external]
```

### 3.8 자기 권한 상승 / 설정 변조 (한계 명시)

```yaml
self_privilege_escalation: # [partial: ACP fs API 또는 terminal 경유 시]
  hard_block:
    - "MCP config mutation"
    - "ACP-mcp permission config mutation"
    - "~/.claude/** write"
    - ".claude/settings.json write"
    - ".claude/settings.local.json write"
    - ".git/hooks/** write"
    - ".husky/** write"
    - "~/.bashrc / ~/.zshrc / ~/.profile / ~/.config/fish/config.fish write"
    - "~/.config/git/** write"
    - "package.json scripts.preinstall/install/postinstall/prepare 신규 추가"
    - ".npmrc / pip.conf write"
    - "pyproject.toml의 build hook 의심 변경"
  limitation: "에이전트가 fs capability를 우회한 경우 OS 샌드박스 read-only mount로 보강 필요." # [external]
```

### 3.9 영속화 / 백그라운드 실행 (시점 한정)

```yaml
persistence:
  hard_block_at_invocation: # [partial: terminal tool 경유 시]
    - "nohup 장기 백그라운드 프로세스"
    - "disown / setsid"
    - "crontab mutation"
    - "systemctl --user"
    - "launchctl / launchd plist 등록"
    - "로그인 항목 / 시작 스크립트 mutation"
    - "kill -9 1"
  limitation: >
    "이미 detach된 백그라운드 프로세스의 사후 추적/종료는 MCP 책임 밖이다.
     세션 종료 시 에이전트 프로세스 트리 전체를 SIGKILL하는 정도의 cleanup만 보장한다."
```

---

## 4~6. Mode 정책 (원문 유지)

원문 그대로 사용 가능. 단, 각 모드의 enforcement는 §0의 visibility_scope를 따른다.
(read-only, edit, full 정의는 원문 §4-§6 동일)

---

## 7. 패키지 설치 정책 (약화)

```yaml
package_install_policy:

  edit_mode:
    default: "confirm"

  full_mode:
    auto_approve_only_if_all_true:
      - "기존 lockfile 존재"
      - "package manager가 lockfile에서 자동 검출됨"
      - "registry가 allowlist 내"
      - "dependency 추가가 아닌 restore/install 재현 성격"
      - "에이전트 명령에 --ignore-scripts 가 포함되어 있음" # CHANGED
      - "lockfile diff 없음 또는 매우 작음"

    still_requires_confirm:
      - "새 dependency 추가"
      - "lockfile 신규 생성 / 대규모 변동"
      - "package manager 변경"
      - "lockfile 없이 npm/pnpm/yarn install"
      - "git URL / tarball URL / file: / http: / local path dependency"
      - "pip install --extra-index-url"
      - "lifecycle script 실행 가능성이 있는 install (--ignore-scripts 미포함)"

  hard_block:
    - "npm install -g, pnpm add -g, yarn global add"
    - "pip install --user, pipx install, uv tool install"
    - "cargo install, go install pkg@version"

  # CHANGED 사유 명시
  changed_from_original:
    rationale: >
      "원문의 '--ignore-scripts 사용 가능'은 정책 엔진이 명령을 rewrite할 수 있다고 가정했으나,
       ACP request_permission은 allow/reject 양자택일이다.
       따라서 자동 삽입은 불가능하고, '에이전트가 보낸 argv에 이미 포함되어 있는지'만 검사한다.
       포함되어 있지 않으면 confirm 경로로 보낸다."

  removed_from_original:
    - item: "lifecycle script 실행 자체 차단"
      reason: "패키지 매니저가 내부적으로 실행. MCP는 매니저 호출 시점만 가시. 사후 hook 실행 차단 불가."
```

---

## 8. 네트워크 정책 (한계 명시)

```yaml
network_policy:
  default: "deny"
  enforcement_scope: "terminal tool로 실행되는 curl/wget/nc/ssh/scp/rsync/git 등에 한함" # NEW

  classification:
    read_only:
      methods: ["GET", "HEAD"]
      body_allowed: false
      file_upload_allowed: false
    write_or_exfil_risk:
      methods: ["POST", "PUT", "PATCH", "DELETE"]
      body_allowed: true
      file_upload_allowed: true

  mode_policy:
    read_only_mode:    { outbound: "deny (all)" }
    edit_mode:         { outbound: "deny_by_default (명시적 read-only allowlist만 허용)" }
    full_mode:
      read_only_to_allowlist: "auto_approve"
      write_or_body_request:  "confirm"
      file_upload_request:    "confirm_or_deny"
      unknown_host:           "deny"

  default_domain_allowlist:
    anthropic: ["api.anthropic.com"]
    github:    ["api.github.com", "raw.githubusercontent.com", "codeload.github.com", "objects.githubusercontent.com"]
    npm:       ["registry.npmjs.org", "registry.npmjs.com", "registry.yarnpkg.com"]
    python:    ["pypi.org", "files.pythonhosted.org"]
    rust:      ["crates.io", "index.crates.io", "static.crates.io"]
    go:        ["proxy.golang.org", "sum.golang.org"]

  out_of_scope: # NEW
    description: >
      "에이전트 프로세스 내부에서 자체 HTTP 클라이언트(fetch/axios/requests 등)로
       발생시키는 outbound 요청은 ACP의 가시범위 밖이며 본 정책 엔진으로 통제 불가능하다."
    defense_in_depth:
      - "에이전트 spawn 시 outbound proxy 환경변수(HTTP_PROXY/HTTPS_PROXY)를 강제 주입"
      - "OS 수준 네트워크 namespace로 allowlist 외 송신 차단"
      - "DNS 차원 allowlist (e.g. CoreDNS, dnsmasq)"
```

---

## 9. MCP Tool Annotation 처리 (원문 유지)

```yaml
mcp_tool_annotation_policy:
  usage:
    - "위험도 초기 분류 hint"
    - "사용자 confirm 프롬프트 설명 자료"
    - "자동 승인 여부의 보조 신호"

  supported_hints: ["readOnlyHint", "destructiveHint", "idempotentHint", "openWorldHint"]

  trust_weight: { trusted_server: "medium", untrusted_server: "low" }
  final_decision_source: "local policy engine"

  default_if_missing:
    readOnlyHint:    false
    destructiveHint: true
    idempotentHint:  false
    openWorldHint:   true

  rule: "annotation은 contract가 아닌 hint다. 충돌 시 정책 엔진이 우선."
```

---

## 10. 구현 체크리스트 (수정)

```yaml
implementation_checklist:

  - id: "P-01"
    rule: "ACP configOptions 우선 + modes 동기화"

  - id: "P-02"
    rule: "세션 내 권한 상승 거부, 축소 허용"
    extra: "current_mode_update notification으로 자율 상승 시도 시, 엔진 내부 mode는 변경하지 않고 audit log에만 기록."

  - id: "P-03"
    rule: "Layer 0 독립 정책 엔진 (pure function)"

  - id: "P-04"
    rule: "argv/AST 기반 명령 검증"
    scope: "ACP terminal/create 요청의 argv에 한함"

  - id: "P-05"
    rule: "shell wrapper 고위험 재분류"
    scope: "위와 동일"

  - id: "P-06"
    rule: "경로 canonicalization"
    detail: "realpath + path separator boundary 비교. Windows/UNC/대소문자 무시 FS 별도 처리."

  - id: "P-07-REMOVED"  # 삭제
    original: "TOCTOU 방어 (openat / O_NOFOLLOW / dirfd)"
    reason: >
      "Node.js는 openat / O_NOFOLLOW를 표준 라이브러리에서 제공하지 않으며,
       실제 fs 작업자가 MCP 서버가 아닌 에이전트 프로세스이므로 정책 엔진 내에서
       TOCTOU-safe하게 강제할 방법이 없다. 이 방어는 (a) 에이전트 측 fs 구현 책임,
       (b) OS 샌드박스(read-only mount, Landlock)로 이관한다."

  - id: "P-08"
    rule: "환경변수 최소화"
    scope: "에이전트 subprocess spawn 시점에만 강제. 실행 중 env 변경은 불가."
    removed_vars: ["GITHUB_TOKEN", "AWS_*", "OPENAI_API_KEY", "ANTHROPIC_API_KEY",
                   "SSH_AUTH_SOCK", "GH_TOKEN", "GITLAB_TOKEN", "NPM_TOKEN"]

  - id: "P-09"
    rule: "네트워크 egress 검증 (4축)"
    scope: "terminal tool 경유 outbound에 한함. 에이전트 내장 HTTP는 외부 메커니즘."

  - id: "P-10"
    rule: "패키지 설치 lifecycle 방어"
    detail: "--ignore-scripts 자동 삽입 불가. 에이전트 argv 검사 후 confirm/deny 양자택일."

  - id: "P-11"
    rule: "chmod permission bit 평가"
    scope: "terminal tool 경유 chmod에 한함."

  - id: "P-12"
    rule: "승인 게이트 타임아웃 = reject"

  - id: "P-13"
    rule: "감사 로그 무결성"

  - id: "P-14"
    rule: "권한 결정 dry-run"

  - id: "P-15"
    rule: "MCP annotation 신뢰 경계 (hint only)"

  - id: "P-16"
    rule: "decision pipeline 고정 순서"

  - id: "P-17"  # NEW
    rule: "Defense-in-depth: 에이전트 spawn 시 OS 샌드박스 적용 권장"
    detail: >
      "Linux Landlock/seccomp/netns, macOS sandbox-exec, container --network=none --read-only.
       MCP 측 정책으로 막을 수 없는 영역(에이전트 자체 fs/net)을 OS 차원에서 보강."
```

---

## 11. 감사 로그 스키마 (원문 유지)

원문 §11 그대로. 추가로 `audit_log_entry.visibility`(`visible | partial | external`) 필드를 추가하여
어떤 가시범위에서 판정된 결정인지 기록한다.

---

## 12. Decision Pipeline (원문 유지)

```python
def decide(request) -> Decision:
    parsed = parse_command(request.raw)
    paths  = canonicalize_paths(request.paths)
    net    = classify_network(request)

    if hit := layer_0_check(parsed, paths, net):
        return Decision.DENY(layer="layer_0", code=hit.code)
    if hit := user_project_deny(parsed, paths, net):
        return Decision.DENY(layer="user_policy", code=hit.code)

    mode_result = current_mode_policy(parsed, paths, net)
    if mode_result.denied:
        return Decision.DENY(layer="mode_policy", code=mode_result.code)

    if mode_result.needs_confirm:
        outcome = send_request_permission(request)
        if outcome in (TIMEOUT, REJECT):
            return Decision.DENY(layer="confirm_gate", code="USER_REJECT_OR_TIMEOUT")
        return Decision.ALLOW(layer="confirm_gate")

    return Decision.ALLOW(layer="mode_policy")
```

---

## 부록 A. 원문 대비 변경 요약

```yaml
removed_or_demoted:
  - id: "P-07 TOCTOU"
    action: "REMOVED (MCP 책임 외 이관)"
    reason: "Node.js openat 미지원 + 실 fs 작업자가 에이전트 프로세스"

  - id: "3.4 denied_indirect_exposure (embedding/indexing/summarization/diff)"
    action: "DEMOTED to best-effort"
    reason: "LLM 내부 처리는 ACP 가시범위 밖"

  - id: "7. --ignore-scripts 자동 삽입"
    action: "REMOVED (argv 검사로 대체)"
    reason: "request_permission은 allow/reject 양자택일, rewrite 불가"

  - id: "10. supply_chain_warning의 lifecycle script 실행 차단"
    action: "REMOVED (명령어 단위 차단으로 한정)"
    reason: "패키지 매니저 내부 실행은 MCP 미가시"

added:
  - id: "§0 trust_model + visibility_scope"
    reason: "정책 엔진이 막을 수 있는 영역과 막을 수 없는 영역을 명문화"

  - id: "P-02 보강 (current_mode_update notification 처리)"
    reason: "ACP 스펙상 에이전트 자율 mode 변경 허용"

  - id: "P-17 OS 샌드박스 권장"
    reason: "정책 엔진 가시범위 밖 위험을 defense-in-depth로 보강"

  - id: "8. network_policy.out_of_scope"
    reason: "에이전트 내장 HTTP는 MCP 미가시 명문화"

  - id: "각 Layer 0 항목에 visibility tag ([visible]/[partial]/[external])"
    reason: "운영자가 각 룰의 강제 범위를 한눈에 파악"

kept_intact:
  - "§1 ACP 세션 설정 모델"
  - "§2 permission_request_policy (옵션, timeout, layer_0_override)"
  - "§3.3 destructive_git"
  - "§3.5 system_modification"
  - "§3.6 credential_changes"
  - "§4-§6 mode 정의"
  - "§9 MCP annotation"
  - "§11 audit log schema"
  - "§12 decision pipeline"
```
