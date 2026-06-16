# buyong-mcp

코딩 에이전트 워크플로를 위한 독립 실행형 로컬 stdio MCP 서버 모노레포. 세 앱은 서로
독립적인 제품이며, 각 앱의 도구·설정·실행 방법은 앱별 문서가 다룬다.

## 앱

| 경로 | 패키지 / 바이너리 | 설명 | 문서 |
| --- | --- | --- | --- |
| `apps/mcp-server` | `@buyong-mcp/acp-bridge` (TS) | ACP로 다른 코딩 에이전트를 read-only 페어로 호출하는 브리지 | [README](apps/mcp-server/README.md) |
| `apps/scout` | `@buyong-mcp/scout` (TS) | zoekt + Universal Ctags 기반 코드 탐색 primitive | [DESIGN](apps/scout/DESIGN.md) |
| `apps/codemap-search` | `codemap-search` (Rust) | tree-sitter + Tantivy BM25 + ripgrep 내장 코드 탐색 서버 | [README](apps/codemap-search/README.md) |

## 개발

TypeScript 앱(`acp-bridge`, `scout`)은 pnpm + Turborepo로 빌드한다.

```bash
pnpm install
pnpm build           # turbo build
pnpm check-types
pnpm lint            # Biome check
```

`codemap-search`는 Cargo 크레이트라 워크스페이스와 별개로 빌드한다.

```bash
cargo build --release --manifest-path apps/codemap-search/Cargo.toml
```

앱별 실행 인자·환경 변수·릴리스는 각 앱 문서를 참조한다.
