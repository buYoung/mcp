# 부록: 처분·반영되지 않은 P1 검토 결과

[미반영 결함 목록](unresolved-defects.md)의 R1·R2 항목 세부다. 대상 데이터셋은 `rts3-c4a3b0fe9eb74bc2`, 기준선은 `rts2-309e9bbaa19f7eb5`이며 2026-10-16에 최종 DB를 직접 조회해 만들었다. 행마다 그 단어의 가장 최근 비교 파일을 기준으로 한다. 모든 조치 원문과 개념 이름은 [unreflected-review-actions.json](unreflected-review-actions.json)에 있다.

비교 파일 경로의 기준은 `~/.cache/related-terms-usage-1/word-eval/followup-20261011/`이다. "합의"는 analyst와 crosscheck 두 검토자가 같은 조치를 낸 것이고, 부모 수락을 뜻하지 않는다.

## 1. 두 검토자가 잘못된 연결이라고 합의했으나 남아 있는 연결

부모 처분 기록이 없는 단어의 합의된 own·input 제거 조치다. 표의 연결은 최종 데이터셋에 그대로 있다.

| 단어 | 종류 | 개념 (이름, 주제) |
| --- | --- | --- |
| `averaged` | input | 2849 (middle, t699.2) |
| `batcher` | own | 739 (cmd, t326.1) |
| `blueprints` | input | 3508 (debug, t347.1) |
| `bubbled` | own | 456 (bubble, t220) |
| `bufferable` | input | 4647 (top, t997) |
| `bufferable` | input | 5123 (vim.lsp.buf.references, t1044.1) |
| `bufferable` | own | 4212 (shared, t946) |
| `cancellable` | own | 3935 (cancel, t912) |
| `channels` | input | 2827 (messages, t285.6) |
| `choices` | input | 5072 (not null, t209.1) |
| `claimed` | input | 3245 (alignment, t200.2) |
| `cleanups` | input | 2982 (required, t789) |
| `cleanups` | input | 4161 (detached, t236.1) |
| `cleanups` | input | 536 (bye, t253) |
| `cleanups` | own | 3039 (css reset, t417.3) |
| `clearable` | input | 4203 (union, t698.1) |
| `clicked` | input | 61 (argparse, t37.1) |
| `clicked` | input | 707 (cli, t37.1) |
| `clicked` | input | 790 (argument, t37.1) |
| `columns` | input | 2250 (check box, t390.1) |
| `concatenates` | own | 4509 (interpolation, t595.1) |
| `configurer` | own | 2179 (image pull backoff, t679.1) |
| `consts` | input | 1813 (function composition, t301.1) |
| `contractless` | input | 236 (expect, t13.1) |
| `contractless` | input | 2387 (toBe, t13.1) |
| `contractless` | input | 2944 (model, t655.2) |
| `contractless` | input | 3572 (Protobuf, t636) |
| `contractless` | input | 3980 (rpc, t636) |
| `contractless` | input | 4071 (cons, t713.2) |
| `contractless` | own | 2296 (interface, t646.2) |
| `contractless` | own | 2317 (precondition, t374.1) |
| `contractless` | own | 692 (bytecode, t313.1) |
| `contractless` | own | 928 (Pact, t388) |
| `creat` | input | 1307 (get_or_create, t487.1) |
| `creat` | input | 1615 (factory method, t395.3) |
| `creat` | input | 3618 (upsert, t487.1) |
| `creat` | input | 4488 (graph database, t977.1) |
| `creat` | own | 1014 (http method, t412.1) |
| `creat` | own | 1076 (schema, t70.1) |
| `creat` | own | 1847 (generate, t181.2) |
| `creat` | own | 2237 (constructor, t395.1) |
| `creat` | own | 2260 (bulk insert, t487.2) |
| `creat` | own | 4318 (spawn, t960) |
| `creat` | own | 5002 (expiresAt, t48.3) |
| `creat` | own | 5011 (getUser, t1035) |
| `creat` | own | 986 (bootstrap, t32.2) |
| `creatable` | own | 5002 (expiresAt, t48.3) |
| `descendent` | input | 2743 (case, t531) |
| `descendent` | input | 3813 (docker compose, t169.4) |
| `dotfiles` | input | 2058 (hidden, t656) |
| `dotted names` | input | 4057 (scatter plot, t101.9) |
| `floating` | own | 4203 (union, t698.1) |
| `flt` | own | 4203 (union, t698.1) |
| `framer` | input | 3252 (getDisplayMedia, t137.2) |
| `framer` | input | 4371 (debugger, t439.1) |
| `framer` | own | 1077 (data frame, t67.1) |
| `framer` | own | 1783 (stack frame, t167.1) |
| `framer` | own | 3660 (fps, t872.1) |
| `framer` | own | 455 (box shadow, t219) |
| `gateways` | own | 1838 (wiki, t82.5) |
| `grantee` | own | 3935 (cancel, t912) |
| `grantor` | own | 3935 (cancel, t912) |
| `guarded` | input | 1433 (verified, t19.1) |
| `guarded` | input | 5133 (public, t821.2) |
| `highlighted` | input | 2212 (watermark, t429.2) |
| `hot keys` | input | 91 (debounce, t53.1) |
| `identifiers` | input | 3222 (dictionary, t817.1) |
| `identities` | input | 918 (content encoding, t385) |
| `images` | input | 4344 (catalog, t74.2) |
| `individually` | own | 1426 (entry, t529) |
| `individually` | own | 1833 (mutation, t616.1) |
| `individually` | own | 5035 (user, t896.1) |
| `inherited` | input | 2542 (extended, t729) |
| `inherited` | input | 445 (markdown, t118.2) |
| `inline scripts` | input | 1299 (show, t484) |
| `inlined` | input | 3618 (upsert, t487.1) |
| `installing` | input | 50 (VS Code, t31.1) |
| `invalidations` | own | 1779 (refactor, t604.1) |
| `join handles` | own | 5007 (expect, t550.2) |
| `latencies` | input | 3474 (better, t848) |
| `microtasks` | own | 1453 (schedule, t165.1) |
| `mocked` | own | 4971 (undo, t500) |
| `namespaced` | input | 3705 (readable stream, t241.1) |
| `nonspace` | own | 1601 (filesystem, t570) |
| `nonspace` | own | 3060 (ns, t508.1) |
| `nonspace` | own | 402 (time complexity, t115.2) |
| `nonspace` | own | 509 (capacity, t243) |
| `orphaned` | input | 748 (co authored by, t145.2) |
| `orphans` | own | 3845 (moved, t897) |
| `paged` | own | 2300 (error page, t456.2) |
| `paged` | own | 3252 (getDisplayMedia, t137.2) |
| `paged` | own | 486 (end to end test, t237.1) |
| `paged` | own | 6981 (web page, t282.1) |
| `participants` | input | 983 (package, t405.1) |
| `patches` | input | 5394 (확장 기여 지점, t244.7) |
| `patching` | input | 5245 (rwlock, t576.2) |
| `paused` | input | 3219 (ended, t177.2) |
| `payment gateways` | own | 4092 (api key, t20.4) |
| `payment intents` | own | 4419 (status, t212.1) |
| `peers` | input | 1723 (fixes, t593) |
| `peers` | input | 4334 (spread, t105.1) |
| `phases` | input | 5095 (secrets manager, t20.6) |
| `popularity` | own | 4941 (frontend, t887) |
| `positioner` | input | 1962 (vertex, t354.1) |
| `positioner` | input | 2178 (layer, t64.1) |
| `positioner` | input | 3177 (order, t276.1) |
| `positioner` | input | 3409 (Intl.PluralRules, t315.5) |
| `prelogin` | own | 1765 (forgot password, t599) |
| `prelogin` | own | 4245 (Firebase Auth, t141.1) |
| `presort` | input | 1372 (classify, t509) |
| `presorted` | input | 1372 (classify, t509) |
| `rating` | input | 4344 (catalog, t74.2) |
| `redisplay` | own | 1024 (Flex, t416) |
| `redisplay` | own | 1298 (screen, t483.1) |
| `redisplay` | own | 1956 (grid, t416) |
| `registered users` | input | 4171 (RegisterService, t941) |
| `remap` | own | 2730 (has, t379.1) |
| `remap` | own | 3183 (sorted map, t152.2) |
| `remap` | own | 5185 (hash map, t446.2) |
| `remover` | own | 1638 (cascade, t577) |
| `resolving` | input | 1662 (cursor, t561.1) |
| `revisioned` | own | 1895 (commit, t140.1) |
| `revisioned` | own | 4206 (sha, t140.1) |
| `revisioned` | own | 5219 (svn, t140.9) |
| `rewire` | own | 1781 (fragmentation, t605) |
| `rewire` | own | 3572 (Protobuf, t636) |
| `rewritten` | input | 1806 (whole, t609) |
| `rewritten` | input | 4863 (transform, t649.1) |
| `rlim` | own | 4667 (recursion, t402.1) |
| `scalars` | input | 3659 (headless, t18.10) |
| `senders` | own | 1191 (shipped, t454) |
| `senders` | own | 2947 (Perl, t784) |
| `senders` | own | 4309 (emitter, t161.1) |
| `sides` | own | 2329 (side effects, t465.2) |
| `sizer` | own | 4275 (offset, t823) |
| `sizer` | own | 4456 (std::queue, t112.1) |
| `slaves` | input | 30 (cors, t16) |
| `slotted` | input | 4261 (only, t287) |
| `sourced` | input | 1250 (diagnostic, t312.2) |
| `sourced` | input | 2313 (convert, t373.1) |
| `sourced` | input | 4035 (sankey, t924) |
| `sourced` | input | 4211 (Object.assign, t782.3) |
| `sourced` | input | 4311 (machine translation, t315.6) |
| `sourced` | input | 4526 (strcpy, t57.2) |
| `sourced` | input | 4533 (clone, t782.3) |
| `sourced` | input | 4644 (Tailwind CSS, t149.2) |
| `sourced` | input | 6007 (Source directories, t405.1) |
| `sourced` | input | 602 (error wrapping, t281.1) |
| `sourced` | own | 180 (Argo CD, t103) |
| `sourced` | own | 2731 (source map, t249.2) |
| `sourced` | own | 3789 (regular expression, t573.1) |
| `sourced` | own | 4685 (tpl, t352.2) |
| `sourced` | own | 5015 (tracking branch, t140.6) |
| `sourced` | own | 5180 (Poetry, t155.2) |
| `sourced` | own | 526 (build system, t181.2) |
| `sourced` | own | 6991 (request binding input, t88.1) |
| `sponsored` | input | 56 (ldap, t34.1) |
| `statics` | own | 2364 (incremental static regeneration, t118.1) |
| `statics` | own | 3384 (pinned, t840.1) |
| `statics` | own | 4880 (static analysis, t251.3) |
| `statics` | own | 5018 (route, t76.3) |
| `statics` | own | 891 (cast, t373.1) |
| `subflow` | own | 3628 (pyi, t125.3) |
| `subtotals` | input | 3459 (defer, t319.2) |
| `subvalue` | own | 3300 (pass by value, t782.1) |
| `subvalue` | own | 5083 (val, t1042) |
| `suggestions` | input | 101 (accept, t15.2) |
| `tapped` | input | 962 (swd, t171.4) |
| `tapped` | own | 3001 (network interface, t791.1) |
| `tapped` | own | 4896 (vpn, t1015) |
| `terminals` | own | 6774 (terminal task state, t212.1) |
| `tokenable` | input | 1597 (expression, t11.5) |
| `tokenable` | input | 1658 (code block, t118.2) |
| `tokenable` | input | 1957 (grammar, t11.2) |
| `tokenable` | input | 2420 (parseerror, t11.9) |
| `tokenable` | input | 2679 (parser combinator, t11.4) |
| `tokenable` | input | 3152 (analyzer, t56.5) |
| `tokenable` | input | 3287 (syntax tree, t11.6) |
| `tokenable` | input | 3341 (parser, t11.4) |
| `tokenable` | input | 3464 (recursive descent, t11.5) |
| `tokenable` | input | 4487 (nlp, t939) |
| `tokenable` | input | 4520 (split, t391.3) |
| `tokenable` | input | 4528 (strtok, t391.3) |
| `tokenable` | input | 4690 (keywords, t56.7) |
| `tokenable` | own | 1370 (XML, t508.1) |
| `tokenable` | own | 1435 (embedding, t184.4) |
| `tokenable` | own | 4485 (max tokens, t184.3) |
| `tokenable` | own | 4616 (token, t11.3) |
| `tokenable` | own | 4819 (design tokens, t164.4) |
| `tokenable` | own | 570 (cancellation token, t269.1) |
| `tokenable` | own | 6277 (context variable reset token, t386) |
| `traceable` | input | 1669 (px.line, t101.6) |
| `traceable` | input | 5412 (차트 데이터 계열, t694.4) |
| `traceable` | own | 1014 (http method, t412.1) |
| `typing` | input | 2354 (IServiceProvider, t43.1) |
| `uncompressed` | input | 3027 (original, t140.8) |
| `uncompressed` | input | 4529 (unpack, t200.3) |
| `underlined` | own | 4453 (span, t187.2) |
| `unparsed` | input | 3027 (original, t140.8) |
| `unshared` | input | 2093 (asg, t459) |
| `unshared` | input | 341 (filepath.Join, t0.1) |
| `unshared` | input | 3459 (defer, t319.2) |
| `unshared` | input | 3681 (ratio, t875) |
| `unshared` | input | 5198 (broadcast, t235.3) |
| `unshared` | input | 821 (partial, t352.1) |
| `unsign` | own | 4245 (Firebase Auth, t141.1) |
| `unslash` | own | 341 (filepath.Join, t0.1) |
| `unslash` | own | 5018 (route, t76.3) |
| `untrash` | input | 3613 (purge, t453.3) |
| `unwatched` | own | 4944 (iOS, t87.7) |
| `value changed` | input | 16 (previous, t9) |
| `valued` | own | 3300 (pass by value, t782.1) |
| `valued` | own | 5083 (val, t1042) |
| `variants` | input | 3239 (imports, t405.1) |
| `variants` | input | 4344 (catalog, t74.2) |
| `verifiable credentials` | input | 2807 (vcs, t140.7) |
| `viewports` | input | 2395 (Jinja, t352.2) |
| `virtualize` | own | 486 (end to end test, t237.1) |
| `waitable` | own | 2040 (watchdog, t169.2) |
| `waitable` | own | 2491 (cooldown, t53.2) |

## 2. 합의됐으나 추가되지 않은 own·개념·이동

| 단어 | 조치 |
| --- | --- |
| `canonicalizes` | own 추가 7204 (request canonicalization, t173.3) |
| `components` | own 추가 6573 (exception grouping components, t694.1) |
| `components` | own 추가 7246 (rule value list, t796.1) |
| `configurer` | own 추가 2079 (callback, t528.3) |
| `configurer` | own 추가 72 (administrator, t44.1) |
| `consoles` | own 추가 7185 (Windows console host, t326.4) |
| `descendent` | own 추가 3956 (tree, t496.4) |
| `flashes` | own 추가 6346 (flash message, t352.2) |
| `fulfillments` | 개념 5487 이동 t213.4→t261.3 |
| `highlighter` | 개념 5944 이동 t13.1→t572 |
| `identifiers` | own 추가 6826 (XML Encryption algorithm identifiers, t80.3) |
| `idxs` | own 추가 2219 (IndexOutOfBoundsException, t684) |
| `incl` | 개념 5532 이동 t89.5→t310.2 |
| `infallible` | own 추가 2199 (impossible, t515.4) |
| `keyed` | own 추가 1911 (provider, t43.4) |
| `keystores` | 개념 5563 이동 t706.4→t20.6 |
| `nonspace` | own 추가 5366 (character classification, t310.8) |
| `normalized` | own 추가 891 (cast, t373.1) |
| `orphaned` | own 추가 6486 (profile inheritance, t276.1) |
| `polls` | own 추가 256 (polling, t130) |
| `projected` | own 추가 4863 (transform, t649.1) |
| `purchasable` | own 추가 3394 (subscription, t111.1) |
| `purchased` | own 추가 3394 (subscription, t111.1) |
| `rates` | own 추가 4664 (tax, t261.2) |
| `ru` | own 추가 3377 (top, t234.4) |
| `sanitizations` | own 추가 6344 (binary structure integrity, t201.1) |
| `scrolling` | own 추가 6770 (terminal scrolling, t415) |
| `sides` | own 추가 465 (padding, t226) |
| `sounds` | 개념 5964 이동 t528.2→t36.2 |
| `sounds` | own 추가 5964 (MIDI notes, t528.2) |
| `specs` | own 추가 1763 (remote config, t597) |
| `spis` | 개념 6871 이동 t314→t42.1 |
| `sponsoring` | own 추가 6196 (subscription sponsorship, t111.1) |
| `states` | 새 개념 "Administrative address subdivisions"(t261.3) |
| `states` | own 추가 1833 (mutation, t616.1) |
| `states` | own 추가 6400 (collaboration awareness, t151.1) |
| `states` | own 추가 6686 (TCP connection state, t65.2) |
| `statics` | 새 개념 "WinRT static-member interfaces"(x24) |
| `stime` | 새 개념 "system CPU time"(t401.6) |
| `subsections` | 새 개념 "form subsection"(t390.1) |
| `subtotals` | own 추가 4842 (total, t50.1) |
| `suggestions` | own 추가 7132 (shell command correction, t326.4) |
| `targets` | own 추가 6166 (business performance goal, t299.1) |
| `texts` | own 추가 2630 (translation, t315.4) |
| `tokenable` | own 추가 6110 (protected token payload, t159.4) |
| `trimmed` | own 추가 3754 (Redis Cluster, t876.2) |
| `typing` | own 추가 3628 (pyi, t125.3) |
| `unloaded` | own 추가 6500 (native control lifecycle, t335.3) |
| `untag` | own 추가 1226 (deserialize, t460.1) |
| `utilities` | own 추가 2048 (test helper, t45.2) |
| `variations` | 개념 6441 이동 t657.1→x41 |
| `waitable` | 새 개념 "waitable synchronization object"(t917.1) |
| `warns` | own 추가 6903 (warning assertions, t13.1) |
| `wcs` | own 추가 4716 (string, t160.8) |
| `wcs` | own 추가 6230 (woocommerce shipping and tax, t261.3) |
| `weighted` | 개념 6670 이동 t81.2→x25 |
| `woken` | own 추가 5170 (waker, t128.6) |
| `zoomed` | 개념 6034 이동 t15.1→t275.3 |

## 3. 단어별 미반영 합의 조치 수

뜻 상속(파생형이 기본형의 특정 개념을 이어받는 조치)은 수가 많아 개수만 적었다. 개별 개념은 JSON 부록에 있다.

| 단어 | 뜻 상속 | own 추가 | own 제거 | input 제거 | 기타 | 이견만 미반영 | 비교 파일 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `attrs` | 2 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-003/p1-base-002-comparison.json` |
| `averaged` | 0 | 0 | 0 | 1 | 0 | 1 | `semantic-comparison-003/p1-base-002-comparison.json` |
| `backends` | 11 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-003/p1-base-002-comparison.json` |
| `batched` | 13 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-003/p1-base-002-comparison.json` |
| `batcher` | 0 | 0 | 1 | 0 | 0 | 0 | `semantic-comparison-003/p1-base-002-comparison.json` |
| `bics` | 1 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-003/p1-base-002-comparison.json` |
| `blinking` | 2 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-003/p1-base-002-comparison.json` |
| `blueprints` | 2 | 0 | 0 | 1 | 0 | 0 | `semantic-comparison-003/p1-base-002-comparison.json` |
| `bracketed` | 1 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-003/p1-base-002-comparison.json` |
| `branding` | 3 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-003/p1-base-002-comparison.json` |
| `brands` | 3 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-003/p1-base-002-comparison.json` |
| `bubbled` | 0 | 0 | 1 | 0 | 0 | 1 | `semantic-comparison-003/p1-base-002-comparison.json` |
| `bufferable` | 0 | 0 | 1 | 2 | 0 | 11 | `semantic-comparison-003/p1-base-002-comparison.json` |
| `bundles` | 14 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-003/p1-base-002-comparison.json` |
| `cancellable` | 0 | 0 | 1 | 0 | 0 | 0 | `semantic-comparison-003/p1-base-002-comparison.json` |
| `canonicalized` | 4 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-003/p1-base-002-comparison.json` |
| `canonicalizes` | 3 | 1 | 0 | 0 | 0 | 1 | `semantic-comparison-004/p1-base-003-1-comparison.json` |
| `cascades` | 8 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-004/p1-base-003-1-comparison.json` |
| `cells` | 8 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-004/p1-base-003-1-comparison.json` |
| `channels` | 31 | 0 | 0 | 1 | 0 | 1 | `semantic-comparison-006/p1-base-003-2-comparison.json` |
| `choices` | 3 | 0 | 0 | 1 | 0 | 0 | `semantic-comparison-006/p1-base-003-2-comparison.json` |
| `claimed` | 2 | 0 | 0 | 1 | 0 | 0 | `semantic-comparison-006/p1-base-003-2-comparison.json` |
| `cleanups` | 0 | 0 | 1 | 3 | 0 | 0 | `semantic-comparison-008/p1-base-003-3a-comparison.json` |
| `clearable` | 0 | 0 | 0 | 1 | 0 | 1 | `semantic-comparison-009/p1-base-003-3b-comparison.json` |
| `clicked` | 0 | 0 | 0 | 3 | 0 | 0 | `semantic-comparison-009/p1-base-003-3b-comparison.json` |
| `cohorts` | 2 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-009/p1-base-003-3b-comparison.json` |
| `collaborators` | 1 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-009/p1-base-003-3b-comparison.json` |
| `collected` | 8 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-010/p1-base-003-4-comparison.json` |
| `cols` | 2 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-010/p1-base-003-4-comparison.json` |
| `columns` | 12 | 0 | 0 | 1 | 0 | 2 | `semantic-comparison-011/p1-base-003-5-comparison.json` |
| `comparers` | 4 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-012/p1-base-004-1-comparison.json` |
| `components` | 9 | 2 | 0 | 0 | 0 | 1 | `semantic-comparison-012/p1-base-004-1-comparison.json` |
| `compressed` | 2 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-012/p1-base-004-1-comparison.json` |
| `concatenates` | 0 | 0 | 1 | 0 | 0 | 0 | `semantic-comparison-013/p1-base-004-2-comparison.json` |
| `configurer` | 0 | 2 | 1 | 0 | 0 | 2 | `semantic-comparison-013/p1-base-004-2-comparison.json` |
| `consoles` | 0 | 1 | 0 | 0 | 0 | 3 | `semantic-comparison-013/p1-base-004-2-comparison.json` |
| `consts` | 0 | 0 | 0 | 1 | 0 | 3 | `semantic-comparison-013/p1-base-004-2-comparison.json` |
| `contractless` | 0 | 0 | 4 | 6 | 0 | 3 | `semantic-comparison-014/p1-base-004-3-comparison.json` |
| `contributes` | 4 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-014/p1-base-004-3-comparison.json` |
| `coordinates` | 3 | 0 | 0 | 0 | 0 | 2 | `semantic-comparison-014/p1-base-004-3-comparison.json` |
| `creat` | 0 | 0 | 9 | 4 | 0 | 1 | `semantic-comparison-016/p1-base-004-5-comparison.json` |
| `creatable` | 0 | 0 | 1 | 0 | 0 | 1 | `semantic-comparison-016/p1-base-004-5-comparison.json` |
| `custom fields` | 1 | 0 | 0 | 0 | 0 | 2 | `semantic-comparison-017/p1-base-005-1-comparison.json` |
| `customizations` | 3 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-017/p1-base-005-1-comparison.json` |
| `data sources` | 11 | 0 | 0 | 0 | 0 | 3 | `semantic-comparison-018/p1-base-005-2-comparison.json` |
| `datasets` | 10 | 0 | 0 | 0 | 0 | 2 | `semantic-comparison-018/p1-base-005-2-comparison.json` |
| `definitions` | 11 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-020/p1-base-005-4-comparison.json` |
| `degrees` | 3 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-020/p1-base-005-4-comparison.json` |
| `deletes` | 32 | 0 | 0 | 0 | 0 | 3 | `semantic-comparison-020/p1-base-005-4-comparison.json` |
| `deletions` | 1 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-021/p1-base-005-5-comparison.json` |
| `descendent` | 0 | 1 | 0 | 2 | 0 | 1 | `semantic-comparison-021/p1-base-005-5-comparison.json` |
| `detectors` | 3 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-021/p1-base-005-5-comparison.json` |
| `devices` | 11 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-021/p1-base-005-5-comparison.json` |
| `directions` | 6 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-022/p1-base-005-6-comparison.json` |
| `dotfiles` | 3 | 0 | 0 | 1 | 0 | 0 | `semantic-comparison-025/p1-base-006-2a-comparison.json` |
| `dotted names` | 1 | 0 | 0 | 1 | 0 | 0 | `semantic-comparison-026/p1-base-006-2b-comparison.json` |
| `fixups` | 3 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-030/p1-base-007-8-comparison.json` |
| `flashes` | 0 | 1 | 0 | 0 | 0 | 0 | `semantic-comparison-030/p1-base-007-8-comparison.json` |
| `floating` | 0 | 0 | 1 | 0 | 0 | 0 | `semantic-comparison-031/p1-base-007-9-comparison.json` |
| `flt` | 0 | 0 | 1 | 0 | 0 | 0 | `semantic-comparison-031/p1-base-008-1-comparison.json` |
| `folders` | 5 | 0 | 0 | 0 | 0 | 2 | `semantic-comparison-031/p1-base-008-2-comparison.json` |
| `fragments` | 15 | 0 | 0 | 0 | 0 | 2 | `semantic-comparison-031/p1-base-008-3-comparison.json` |
| `framed` | 7 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-031/p1-base-008-3-comparison.json` |
| `framer` | 0 | 0 | 4 | 2 | 0 | 6 | `semantic-comparison-031/p1-base-008-3-comparison.json` |
| `fulfillments` | 0 | 0 | 0 | 0 | 1 | 0 | `semantic-comparison-031/p1-base-008-4-comparison.json` |
| `gateways` | 0 | 0 | 1 | 0 | 0 | 1 | `semantic-comparison-031/p1-base-008-6-comparison.json` |
| `generators` | 16 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-031/p1-base-008-6-comparison.json` |
| `grained` | 1 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-031/p1-base-008-7-comparison.json` |
| `grantee` | 0 | 0 | 1 | 0 | 0 | 2 | `semantic-comparison-031/p1-base-008-7-comparison.json` |
| `grantor` | 0 | 0 | 1 | 0 | 0 | 0 | `semantic-comparison-031/p1-base-008-8-comparison.json` |
| `grouping` | 6 | 0 | 0 | 0 | 0 | 3 | `semantic-comparison-031/p1-base-008-8-comparison.json` |
| `guarded` | 12 | 0 | 0 | 2 | 0 | 1 | `semantic-comparison-031/p1-base-008-8-comparison.json` |
| `guards` | 15 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-032/p1-base-008-9-comparison.json` |
| `highlighted` | 5 | 0 | 0 | 1 | 0 | 2 | `semantic-comparison-032/p1-base-009-2-comparison.json` |
| `highlighter` | 0 | 0 | 0 | 0 | 1 | 3 | `semantic-comparison-032/p1-base-009-2-comparison.json` |
| `highlights` | 4 | 0 | 0 | 0 | 0 | 3 | `semantic-comparison-032/p1-base-009-2-comparison.json` |
| `hot keys` | 2 | 0 | 0 | 1 | 0 | 1 | `semantic-comparison-032/p1-base-009-3-comparison.json` |
| `identifiers` | 12 | 1 | 0 | 1 | 0 | 2 | `semantic-comparison-032/p1-base-009-4-comparison.json` |
| `identities` | 0 | 0 | 0 | 1 | 0 | 2 | `semantic-comparison-032/p1-base-009-5-comparison.json` |
| `idioms` | 2 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-032/p1-base-009-5-comparison.json` |
| `idxs` | 0 | 1 | 0 | 0 | 0 | 1 | `semantic-comparison-032/p1-base-009-5-comparison.json` |
| `images` | 17 | 0 | 0 | 1 | 0 | 2 | `semantic-comparison-032/p1-base-009-6-comparison.json` |
| `imported` | 16 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-032/p1-base-009-6-comparison.json` |
| `importing` | 16 | 0 | 0 | 0 | 0 | 2 | `semantic-comparison-032/p1-base-009-7-comparison.json` |
| `incl` | 0 | 0 | 0 | 0 | 1 | 1 | `semantic-comparison-032/p1-base-009-7-comparison.json` |
| `individually` | 0 | 0 | 3 | 0 | 0 | 0 | `semantic-comparison-032/p1-base-009-8-comparison.json` |
| `infallible` | 0 | 1 | 0 | 0 | 0 | 0 | `semantic-comparison-032/p1-base-009-8-comparison.json` |
| `infos` | 4 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-032/p1-base-009-8-comparison.json` |
| `inherited` | 10 | 0 | 0 | 2 | 0 | 2 | `semantic-comparison-032/p1-base-009-9-comparison.json` |
| `inline scripts` | 1 | 0 | 0 | 1 | 0 | 0 | `semantic-comparison-032/p1-base-010-1-comparison.json` |
| `inlined` | 8 | 0 | 0 | 1 | 0 | 4 | `semantic-comparison-032/p1-base-010-1-comparison.json` |
| `insights` | 2 | 0 | 0 | 0 | 0 | 7 | `semantic-comparison-032/p1-base-010-1-comparison.json` |
| `installing` | 11 | 0 | 0 | 1 | 0 | 0 | `semantic-comparison-032/p1-base-010-2-comparison.json` |
| `interested` | 1 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-032/p1-base-010-2-comparison.json` |
| `interests` | 1 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-032/p1-base-010-3-comparison.json` |
| `invalidations` | 0 | 0 | 1 | 0 | 0 | 1 | `semantic-comparison-032/p1-base-010-3-comparison.json` |
| `join handles` | 0 | 0 | 1 | 0 | 0 | 0 | `semantic-comparison-032/p1-base-010-4-comparison.json` |
| `keyed` | 0 | 1 | 0 | 0 | 0 | 3 | `semantic-comparison-032/p1-base-010-5-comparison.json` |
| `keystores` | 0 | 0 | 0 | 0 | 1 | 0 | `semantic-comparison-032/p1-base-010-6-comparison.json` |
| `kinesis streams` | 1 | 0 | 0 | 0 | 0 | 5 | `semantic-comparison-032/p1-base-010-6-comparison.json` |
| `latencies` | 0 | 0 | 0 | 1 | 0 | 2 | `semantic-comparison-032/p1-base-010-7-comparison.json` |
| `linked issues` | 1 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-032/p1-base-011-2-comparison.json` |
| `loaders` | 10 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-032/p1-base-011-3-comparison.json` |
| `managers` | 8 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-032/p1-base-011-3-comparison.json` |
| `margins` | 7 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-032/p1-base-011-4-comparison.json` |
| `markers` | 8 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-032/p1-base-011-4-comparison.json` |
| `meanings` | 2 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-032/p1-base-011-6-comparison.json` |
| `measured` | 8 | 0 | 0 | 0 | 0 | 3 | `semantic-comparison-032/p1-base-011-6-comparison.json` |
| `measurements` | 5 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-032/p1-base-011-6-comparison.json` |
| `megabytes` | 1 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-032/p1-base-011-7-comparison.json` |
| `menus` | 6 | 0 | 0 | 0 | 0 | 3 | `semantic-comparison-032/p1-base-011-7-comparison.json` |
| `messaging` | 11 | 0 | 0 | 0 | 0 | 3 | `semantic-comparison-032/p1-base-011-8-comparison.json` |
| `microtasks` | 0 | 0 | 1 | 0 | 0 | 0 | `semantic-comparison-032/p1-base-011-9-comparison.json` |
| `migrating` | 5 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-033/p1-base-012-1-comparison.json` |
| `mocked` | 0 | 0 | 1 | 0 | 0 | 0 | `semantic-comparison-033/p1-base-012-2-comparison.json` |
| `module paths` | 4 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-033/p1-base-012-3-comparison.json` |
| `multiple choices` | 1 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-033/p1-base-012-4-comparison.json` |
| `multivalued` | 1 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-033/p1-base-012-4-comparison.json` |
| `muted` | 1 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-033/p1-base-012-4-comparison.json` |
| `named queries` | 2 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-033/p1-base-012-5-comparison.json` |
| `namespaced` | 27 | 0 | 0 | 1 | 0 | 1 | `semantic-comparison-033/p1-base-012-5-comparison.json` |
| `nonspace` | 0 | 1 | 4 | 0 | 0 | 1 | `semantic-comparison-033/p1-base-012-6-comparison.json` |
| `normalized` | 14 | 1 | 0 | 0 | 0 | 3 | `semantic-comparison-033/p1-base-012-6-comparison.json` |
| `not allowed` | 1 | 0 | 0 | 0 | 0 | 2 | `semantic-comparison-033/p1-base-012-7-comparison.json` |
| `notifying` | 12 | 0 | 0 | 0 | 0 | 3 | `semantic-comparison-033/p1-base-012-7-comparison.json` |
| `orphaned` | 3 | 1 | 0 | 1 | 0 | 1 | `semantic-comparison-033/p1-base-012-9-comparison.json` |
| `orphans` | 0 | 0 | 1 | 0 | 0 | 0 | `semantic-comparison-033/p1-base-013-1-comparison.json` |
| `overflows` | 43 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-033/p1-base-013-1-comparison.json` |
| `packages` | 15 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-033/p1-base-013-2-comparison.json` |
| `packets` | 5 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-033/p1-base-013-2-comparison.json` |
| `paged` | 0 | 0 | 4 | 0 | 0 | 2 | `semantic-comparison-033/p1-base-013-3-comparison.json` |
| `participants` | 2 | 0 | 0 | 1 | 0 | 0 | `semantic-comparison-033/p1-base-013-4-comparison.json` |
| `patches` | 28 | 0 | 0 | 1 | 0 | 0 | `semantic-comparison-033/p1-base-013-4-comparison.json` |
| `patching` | 26 | 0 | 0 | 1 | 0 | 1 | `semantic-comparison-033/p1-base-013-5-comparison.json` |
| `paused` | 5 | 0 | 0 | 1 | 0 | 1 | `semantic-comparison-033/p1-base-013-5-comparison.json` |
| `payment gateways` | 0 | 0 | 1 | 0 | 0 | 1 | `semantic-comparison-033/p1-base-013-5-comparison.json` |
| `payment intents` | 0 | 0 | 1 | 0 | 0 | 0 | `semantic-comparison-033/p1-base-013-6-comparison.json` |
| `peers` | 8 | 0 | 0 | 2 | 0 | 0 | `semantic-comparison-033/p1-base-013-6-comparison.json` |
| `phases` | 5 | 0 | 0 | 1 | 0 | 0 | `semantic-comparison-033/p1-base-013-7-comparison.json` |
| `picks` | 5 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-033/p1-base-013-8-comparison.json` |
| `pointer entered` | 1 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-033/p1-base-013-8-comparison.json` |
| `polls` | 0 | 1 | 0 | 0 | 0 | 0 | `semantic-comparison-033/p1-base-014-2-comparison.json` |
| `popularity` | 0 | 0 | 1 | 0 | 0 | 2 | `semantic-comparison-033/p1-base-014-2-comparison.json` |
| `popups` | 3 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-033/p1-base-014-2-comparison.json` |
| `positioner` | 0 | 0 | 0 | 4 | 0 | 12 | `semantic-comparison-033/p1-base-014-3-comparison.json` |
| `prelogin` | 0 | 0 | 2 | 0 | 0 | 3 | `semantic-comparison-033/p1-base-014-5-comparison.json` |
| `preprocessed` | 9 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-033/p1-base-014-5-comparison.json` |
| `preprocessing` | 1 | 0 | 0 | 0 | 0 | 3 | `semantic-comparison-033/p1-base-014-5-comparison.json` |
| `presort` | 0 | 0 | 0 | 1 | 0 | 3 | `semantic-comparison-033/p1-base-014-6-comparison.json` |
| `presorted` | 0 | 0 | 0 | 1 | 0 | 3 | `semantic-comparison-033/p1-base-014-6-comparison.json` |
| `previews` | 6 | 0 | 0 | 0 | 0 | 2 | `semantic-comparison-033/p1-base-014-6-comparison.json` |
| `prioritized` | 1 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-034/p1-base-015-2-comparison.json` |
| `processors` | 6 | 0 | 0 | 0 | 0 | 7 | `semantic-comparison-034/p1-base-015-2-comparison.json` |
| `product categories` | 1 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-034/p1-base-015-2-comparison.json` |
| `product types` | 4 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-034/p1-base-015-3-comparison.json` |
| `projected` | 2 | 1 | 0 | 0 | 0 | 2 | `semantic-comparison-034/p1-base-015-3-comparison.json` |
| `promoted` | 4 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-034/p1-base-015-4-comparison.json` |
| `proofs` | 4 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-034/p1-base-015-4-comparison.json` |
| `prorations` | 1 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-034/p1-base-015-4-comparison.json` |
| `public keys` | 11 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-034/p1-base-015-5-comparison.json` |
| `purchasable` | 0 | 1 | 0 | 0 | 0 | 0 | `semantic-comparison-034/p1-base-015-6-comparison.json` |
| `purchased` | 0 | 1 | 0 | 0 | 0 | 0 | `semantic-comparison-034/p1-base-015-6-comparison.json` |
| `purges` | 10 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-034/p1-base-015-6-comparison.json` |
| `qualifiers` | 4 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-034/p1-base-015-7-comparison.json` |
| `quick fixes` | 6 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-034/p1-base-015-7-comparison.json` |
| `rates` | 0 | 1 | 0 | 0 | 0 | 1 | `semantic-comparison-034/p1-base-015-8-comparison.json` |
| `rating` | 0 | 0 | 0 | 1 | 0 | 0 | `semantic-comparison-034/p1-base-016-1-comparison.json` |
| `ratings` | 3 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-034/p1-base-016-1-comparison.json` |
| `react testing library` | 1 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-034/p1-base-016-2-comparison.json` |
| `rects` | 7 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-034/p1-base-016-4-comparison.json` |
| `redisplay` | 0 | 0 | 3 | 0 | 0 | 4 | `semantic-comparison-034/p1-base-016-5-comparison.json` |
| `reflectors` | 1 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-034/p1-base-016-6-comparison.json` |
| `refreshes` | 14 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-034/p1-base-016-6-comparison.json` |
| `registered users` | 0 | 0 | 0 | 1 | 0 | 1 | `semantic-comparison-034/p1-base-016-6-comparison.json` |
| `registries` | 25 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-035/p1-base-016-7-comparison.json` |
| `remap` | 0 | 0 | 3 | 0 | 0 | 4 | `semantic-comparison-035/p1-base-016-7-comparison.json` |
| `remappings` | 1 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-034/p1-base-016-8-comparison.json` |
| `remover` | 0 | 0 | 1 | 0 | 0 | 0 | `semantic-comparison-035/p1-base-017-1-comparison.json` |
| `replacements` | 4 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-035/p1-base-017-1-comparison.json` |
| `replays` | 7 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-034/p1-base-017-2-comparison.json` |
| `replicated` | 4 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-034/p1-base-017-2-comparison.json` |
| `reply to addresses` | 1 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-034/p1-base-017-2-comparison.json` |
| `representations` | 6 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-035/p1-base-017-3-comparison.json` |
| `reprocessing` | 3 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-035/p1-base-017-3-comparison.json` |
| `resolvers` | 12 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-035/p1-base-017-4-comparison.json` |
| `resolving` | 27 | 0 | 0 | 1 | 0 | 2 | `semantic-comparison-035/p1-base-017-5-comparison.json` |
| `resource types` | 2 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-035/p1-base-017-5-comparison.json` |
| `restrictions` | 4 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-035/p1-base-017-5-comparison.json` |
| `reviews` | 3 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-035/p1-base-017-6-comparison.json` |
| `revisioned` | 0 | 0 | 3 | 0 | 0 | 0 | `semantic-comparison-035/p1-base-017-6-comparison.json` |
| `revisions` | 11 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-036/p1-base-017-7-comparison.json` |
| `rewire` | 0 | 0 | 2 | 0 | 0 | 1 | `semantic-comparison-036/p1-base-017-7-comparison.json` |
| `rewrites` | 10 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-035/p1-base-017-8-comparison.json` |
| `rewritten` | 0 | 0 | 0 | 2 | 0 | 1 | `semantic-comparison-035/p1-base-017-8-comparison.json` |
| `rlim` | 0 | 0 | 1 | 0 | 0 | 1 | `semantic-comparison-036/p1-base-017-9-comparison.json` |
| `rngs` | 1 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-038/p1-base-018-1-comparison.json` |
| `ru` | 0 | 1 | 0 | 0 | 0 | 1 | `semantic-comparison-038/p1-base-018-1-comparison.json` |
| `samples` | 6 | 0 | 0 | 0 | 0 | 2 | `semantic-comparison-038/p1-base-018-2-comparison.json` |
| `sanitizations` | 0 | 1 | 0 | 0 | 0 | 0 | `semantic-comparison-038/p1-base-018-2-comparison.json` |
| `scalars` | 7 | 0 | 0 | 1 | 0 | 3 | `semantic-comparison-037/p1-base-018-3-comparison.json` |
| `schemes` | 6 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-038/p1-base-018-4-comparison.json` |
| `scrolling` | 0 | 1 | 0 | 0 | 0 | 0 | `semantic-comparison-038/p1-base-018-4-comparison.json` |
| `senders` | 0 | 0 | 3 | 0 | 0 | 0 | `semantic-comparison-038/p1-base-018-7-comparison.json` |
| `services` | 21 | 0 | 0 | 0 | 0 | 3 | `semantic-comparison-038/p1-base-018-7-comparison.json` |
| `setup intents` | 3 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-037/p1-base-018-8-comparison.json` |
| `short date string` | 3 | 0 | 0 | 0 | 0 | 2 | `semantic-comparison-037/p1-base-018-8-comparison.json` |
| `sides` | 0 | 1 | 1 | 0 | 0 | 0 | `semantic-comparison-037/p1-base-019-1-comparison.json` |
| `signed cookies` | 4 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-037/p1-base-019-1-comparison.json` |
| `sizer` | 0 | 0 | 2 | 0 | 0 | 3 | `semantic-comparison-037/p1-base-019-2-comparison.json` |
| `sizes` | 17 | 0 | 0 | 0 | 0 | 2 | `semantic-comparison-037/p1-base-019-2-comparison.json` |
| `slaves` | 0 | 0 | 0 | 1 | 0 | 1 | `semantic-comparison-037/p1-base-019-3-comparison.json` |
| `slotted` | 0 | 0 | 0 | 1 | 0 | 0 | `semantic-comparison-037/p1-base-019-3-comparison.json` |
| `snippets` | 2 | 0 | 0 | 0 | 0 | 2 | `semantic-comparison-038/p1-base-019-4-comparison.json` |
| `sounds` | 2 | 1 | 0 | 0 | 1 | 0 | `semantic-comparison-038/p1-base-019-4-comparison.json` |
| `source files` | 7 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-037/p1-base-019-5-comparison.json` |
| `sourced` | 0 | 0 | 8 | 10 | 0 | 8 | `semantic-comparison-037/p1-base-019-5-comparison.json` |
| `sources` | 33 | 0 | 0 | 0 | 0 | 9 | `semantic-comparison-037/p1-base-019-5-comparison.json` |
| `spaced` | 1 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-038/p1-base-019-6-comparison.json` |
| `specifiers` | 1 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-038/p1-base-019-6-comparison.json` |
| `specs` | 6 | 1 | 0 | 0 | 0 | 4 | `semantic-comparison-038/p1-base-019-6-comparison.json` |
| `spis` | 7 | 0 | 0 | 0 | 1 | 1 | `semantic-comparison-038/p1-base-019-7-comparison.json` |
| `sponsored` | 2 | 0 | 0 | 1 | 0 | 0 | `semantic-comparison-038/p1-base-019-7-comparison.json` |
| `sponsoring` | 0 | 1 | 0 | 0 | 0 | 0 | `semantic-comparison-038/p1-base-019-7-comparison.json` |
| `stars` | 2 | 0 | 0 | 0 | 0 | 4 | `semantic-comparison-037/p1-base-019-8-comparison.json` |
| `stashing` | 3 | 0 | 0 | 0 | 0 | 2 | `semantic-comparison-037/p1-base-019-9-comparison.json` |
| `states` | 40 | 3 | 0 | 0 | 1 | 0 | `semantic-comparison-037/p1-base-020-1-comparison.json` |
| `statics` | 0 | 0 | 5 | 0 | 1 | 0 | `semantic-comparison-037/p1-base-020-1-comparison.json` |
| `stepped` | 7 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-037/p1-base-020-2-comparison.json` |
| `stime` | 0 | 0 | 0 | 0 | 1 | 0 | `semantic-comparison-037/p1-base-020-2-comparison.json` |
| `string values` | 2 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-037/p1-base-020-3-comparison.json` |
| `stubs` | 9 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-037/p1-base-020-3-comparison.json` |
| `subflow` | 0 | 0 | 1 | 0 | 0 | 0 | `semantic-comparison-038/p1-base-020-4-comparison.json` |
| `subkeys` | 2 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-038/p1-base-020-4-comparison.json` |
| `subsections` | 0 | 0 | 0 | 0 | 1 | 0 | `semantic-comparison-038/p1-base-020-5-comparison.json` |
| `subtotals` | 0 | 1 | 0 | 1 | 0 | 1 | `semantic-comparison-037/p1-base-020-6-comparison.json` |
| `subvalue` | 0 | 0 | 2 | 0 | 0 | 5 | `semantic-comparison-037/p1-base-020-6-comparison.json` |
| `suggestions` | 7 | 1 | 0 | 1 | 0 | 3 | `semantic-comparison-037/p1-base-020-6-comparison.json` |
| `sweeps` | 2 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-038/p1-base-020-7-comparison.json` |
| `synced` | 5 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-038/p1-base-020-8-comparison.json` |
| `tapped` | 0 | 0 | 2 | 1 | 0 | 0 | `semantic-comparison-038/p1-base-020-8-comparison.json` |
| `targets` | 24 | 1 | 0 | 0 | 0 | 0 | `semantic-comparison-038/p1-base-020-8-comparison.json` |
| `taxonomies` | 5 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-039/p1-base-021-1-comparison.json` |
| `terminals` | 0 | 0 | 1 | 0 | 0 | 0 | `semantic-comparison-039/p1-base-021-2-comparison.json` |
| `texts` | 0 | 1 | 0 | 0 | 0 | 0 | `semantic-comparison-039/p1-base-021-2-comparison.json` |
| `tokenable` | 0 | 1 | 7 | 13 | 0 | 2 | `semantic-comparison-040/p1-base-021-4-comparison.json` |
| `tool results` | 1 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-040/p1-base-021-4-comparison.json` |
| `total discount amounts` | 2 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-040/p1-base-021-4-comparison.json` |
| `trace events` | 1 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-040/p1-base-021-5-comparison.json` |
| `trace logging` | 1 | 0 | 0 | 0 | 0 | 2 | `semantic-comparison-040/p1-base-021-5-comparison.json` |
| `traceable` | 0 | 0 | 1 | 2 | 0 | 0 | `semantic-comparison-040/p1-base-021-5-comparison.json` |
| `traits` | 10 | 0 | 0 | 0 | 0 | 3 | `semantic-comparison-039/p1-base-021-6-comparison.json` |
| `transforms` | 26 | 0 | 0 | 0 | 0 | 2 | `semantic-comparison-039/p1-base-022-1-comparison.json` |
| `translators` | 3 | 0 | 0 | 0 | 0 | 3 | `semantic-comparison-039/p1-base-022-1-comparison.json` |
| `transports` | 4 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-039/p1-base-022-1-comparison.json` |
| `trapping` | 5 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-040/p1-base-022-2-comparison.json` |
| `trimmed` | 0 | 1 | 0 | 0 | 0 | 1 | `semantic-comparison-040/p1-base-022-2-comparison.json` |
| `trimming` | 4 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-040/p1-base-022-2-comparison.json` |
| `trust managers` | 2 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-039/p1-base-022-3-comparison.json` |
| `typing` | 0 | 1 | 0 | 1 | 0 | 1 | `semantic-comparison-039/p1-base-022-3-comparison.json` |
| `unassigned` | 1 | 0 | 0 | 0 | 0 | 2 | `semantic-comparison-039/p1-base-022-4-comparison.json` |
| `unblocked` | 2 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-039/p1-base-022-4-comparison.json` |
| `uncompressed` | 1 | 0 | 0 | 2 | 0 | 0 | `semantic-comparison-039/p1-base-022-4-comparison.json` |
| `underlined` | 0 | 0 | 1 | 0 | 0 | 0 | `semantic-comparison-039/p1-base-022-5-comparison.json` |
| `units` | 9 | 0 | 0 | 0 | 0 | 3 | `semantic-comparison-040/p1-base-022-6-comparison.json` |
| `unlinked` | 5 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-040/p1-base-022-6-comparison.json` |
| `unloaded` | 0 | 1 | 0 | 0 | 0 | 0 | `semantic-comparison-040/p1-base-022-6-comparison.json` |
| `unparsed` | 1 | 0 | 0 | 1 | 0 | 1 | `semantic-comparison-039/p1-base-022-7-comparison.json` |
| `unshared` | 0 | 0 | 0 | 6 | 0 | 2 | `semantic-comparison-039/p1-base-022-7-comparison.json` |
| `unsign` | 0 | 0 | 1 | 0 | 0 | 0 | `semantic-comparison-040/p1-base-022-8-comparison.json` |
| `unslash` | 0 | 0 | 2 | 0 | 0 | 1 | `semantic-comparison-040/p1-base-022-8-comparison.json` |
| `unsubscribed` | 13 | 0 | 0 | 0 | 0 | 2 | `semantic-comparison-040/p1-base-022-8-comparison.json` |
| `untag` | 0 | 1 | 0 | 0 | 0 | 1 | `semantic-comparison-040/p1-base-023-1-comparison.json` |
| `untrash` | 0 | 0 | 0 | 1 | 0 | 3 | `semantic-comparison-040/p1-base-023-1-comparison.json` |
| `unwatched` | 0 | 0 | 1 | 0 | 0 | 0 | `semantic-comparison-040/p1-base-023-1-comparison.json` |
| `upsells` | 1 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-041/p1-base-023-2-comparison.json` |
| `utilities` | 2 | 1 | 0 | 0 | 0 | 0 | `semantic-comparison-041/p1-base-023-3-comparison.json` |
| `value changed` | 1 | 0 | 0 | 1 | 0 | 1 | `semantic-comparison-041/p1-base-023-4-comparison.json` |
| `valued` | 0 | 0 | 2 | 0 | 0 | 5 | `semantic-comparison-041/p1-base-023-4-comparison.json` |
| `variants` | 14 | 0 | 0 | 2 | 0 | 3 | `semantic-comparison-041/p1-base-023-4-comparison.json` |
| `variations` | 3 | 0 | 0 | 0 | 1 | 0 | `semantic-comparison-041/p1-base-023-5-comparison.json` |
| `vectored` | 1 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-041/p1-base-023-5-comparison.json` |
| `verifiable credentials` | 0 | 0 | 0 | 1 | 0 | 0 | `semantic-comparison-041/p1-base-023-5-comparison.json` |
| `viewers` | 4 | 0 | 0 | 0 | 0 | 1 | `semantic-comparison-041/p1-base-023-6-comparison.json` |
| `viewports` | 0 | 0 | 0 | 1 | 0 | 0 | `semantic-comparison-041/p1-base-023-6-comparison.json` |
| `virtualize` | 0 | 0 | 1 | 0 | 0 | 1 | `semantic-comparison-041/p1-base-023-7-comparison.json` |
| `waitable` | 0 | 0 | 2 | 0 | 1 | 2 | `semantic-comparison-041/p1-base-024-1-comparison.json` |
| `waiters` | 1 | 0 | 0 | 0 | 0 | 2 | `semantic-comparison-041/p1-base-024-1-comparison.json` |
| `walks` | 6 | 0 | 0 | 0 | 0 | 2 | `semantic-comparison-041/p1-base-024-2-comparison.json` |
| `warns` | 0 | 1 | 0 | 0 | 0 | 0 | `semantic-comparison-041/p1-base-024-3-comparison.json` |
| `wcs` | 0 | 2 | 0 | 0 | 0 | 1 | `semantic-comparison-041/p1-base-024-3-comparison.json` |
| `weighted` | 7 | 0 | 0 | 0 | 1 | 4 | `semantic-comparison-041/p1-base-024-4-comparison.json` |
| `windowed` | 9 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-041/p1-base-024-4-comparison.json` |
| `windowing` | 3 | 0 | 0 | 0 | 0 | 4 | `semantic-comparison-041/p1-base-024-5-comparison.json` |
| `woken` | 0 | 1 | 0 | 0 | 0 | 0 | `semantic-comparison-041/p1-base-024-5-comparison.json` |
| `zones` | 5 | 0 | 0 | 0 | 0 | 0 | `semantic-comparison-041/p1-base-024-6-comparison.json` |
| `zoomed` | 0 | 0 | 0 | 0 | 1 | 0 | `semantic-comparison-041/p1-base-024-7-comparison.json` |

## 4. 합의는 없고 두 검토자의 의견이 갈린 채 남은 단어

중재·부모 선택이 필요하다. 이 목록의 조치는 반영 여부를 판단할 근거가 아직 없다.

| 단어 | 미반영 이견 조치 수 | 비교 파일 |
| --- | ---: | --- |
| `autosaves` | 1 | `semantic-comparison-003/p1-base-002-comparison.json` |
| `avgs` | 1 | `semantic-comparison-003/p1-base-002-comparison.json` |
| `cacher` | 1 | `semantic-comparison-003/p1-base-002-comparison.json` |
| `catcher` | 1 | `semantic-comparison-004/p1-base-003-1-comparison.json` |
| `clamped` | 1 | `semantic-comparison-006/p1-base-003-2-comparison.json` |
| `colons` | 1 | `semantic-comparison-010/p1-base-003-4-comparison.json` |
| `completed task` | 8 | `semantic-comparison-012/p1-base-004-1-comparison.json` |
| `corners` | 2 | `semantic-comparison-015/p1-base-004-4-comparison.json` |
| `crashing` | 1 | `semantic-comparison-016/p1-base-004-5-comparison.json` |
| `cvt` | 5 | `semantic-comparison-017/p1-base-005-1-comparison.json` |
| `dbs` | 1 | `semantic-comparison-019/p1-base-005-3-comparison.json` |
| `disconnection` | 1 | `semantic-comparison-024/p1-base-006-1-comparison.json` |
| `dismissal` | 4 | `semantic-comparison-024/p1-base-006-1-comparison.json` |
| `dismissals` | 4 | `semantic-comparison-024/p1-base-006-1-comparison.json` |
| `flashed` | 1 | `semantic-comparison-030/p1-base-007-8-comparison.json` |
| `fmadd` | 3 | `semantic-comparison-031/p1-base-008-2-comparison.json` |
| `froze` | 1 | `semantic-comparison-031/p1-base-008-4-comparison.json` |
| `hijacked` | 1 | `semantic-comparison-032/p1-base-009-3-comparison.json` |
| `hovered` | 4 | `semantic-comparison-032/p1-base-009-3-comparison.json` |
| `impersonated` | 2 | `semantic-comparison-032/p1-base-009-6-comparison.json` |
| `installations` | 5 | `semantic-comparison-032/p1-base-010-2-comparison.json` |
| `invalidator` | 4 | `semantic-comparison-032/p1-base-010-3-comparison.json` |
| `investigations` | 3 | `semantic-comparison-032/p1-base-010-4-comparison.json` |
| `issuing` | 2 | `semantic-comparison-032/p1-base-010-4-comparison.json` |
| `killable` | 3 | `semantic-comparison-032/p1-base-010-6-comparison.json` |
| `lagging` | 1 | `semantic-comparison-032/p1-base-010-7-comparison.json` |
| `lockouts` | 1 | `semantic-comparison-032/p1-base-011-3-comparison.json` |
| `max permits` | 1 | `semantic-comparison-032/p1-base-011-5-comparison.json` |
| `member names` | 4 | `semantic-comparison-032/p1-base-011-7-comparison.json` |
| `navigated` | 2 | `semantic-comparison-033/p1-base-012-5-comparison.json` |
| `negations` | 1 | `semantic-comparison-033/p1-base-012-6-comparison.json` |
| `notifiable` | 1 | `semantic-comparison-033/p1-base-012-7-comparison.json` |
| `obfuscated` | 1 | `semantic-comparison-033/p1-base-012-8-comparison.json` |
| `overindented` | 1 | `semantic-comparison-033/p1-base-013-1-comparison.json` |
| `paginating` | 1 | `semantic-comparison-033/p1-base-013-3-comparison.json` |
| `pfn` | 1 | `semantic-comparison-033/p1-base-013-7-comparison.json` |
| `pointer moved` | 3 | `semantic-comparison-033/p1-base-014-1-comparison.json` |
| `polled` | 2 | `semantic-comparison-033/p1-base-014-1-comparison.json` |
| `preds` | 4 | `semantic-comparison-033/p1-base-014-3-comparison.json` |
| `progressed` | 3 | `semantic-comparison-034/p1-base-015-3-comparison.json` |
| `proxify` | 1 | `semantic-comparison-034/p1-base-015-5-comparison.json` |
| `reattach` | 1 | `semantic-comparison-034/p1-base-016-3-comparison.json` |
| `recurses` | 1 | `semantic-comparison-034/p1-base-016-4-comparison.json` |
| `renamer` | 1 | `semantic-comparison-035/p1-base-017-1-comparison.json` |
| `rethrows` | 1 | `semantic-comparison-035/p1-base-017-6-comparison.json` |
| `revokable` | 1 | `semantic-comparison-036/p1-base-017-7-comparison.json` |
| `rotated` | 1 | `semantic-comparison-038/p1-base-018-1-comparison.json` |
| `rxs` | 1 | `semantic-comparison-038/p1-base-018-2-comparison.json` |
| `savings` | 4 | `semantic-comparison-037/p1-base-018-3-comparison.json` |
| `selling` | 1 | `semantic-comparison-040/p1-base-018-6-comparison.json` |
| `service accounts` | 1 | `semantic-comparison-038/p1-base-018-7-comparison.json` |
| `signs` | 1 | `semantic-comparison-037/p1-base-019-1-comparison.json` |
| `sliced` | 1 | `semantic-comparison-037/p1-base-019-3-comparison.json` |
| `soft deleted` | 1 | `semantic-comparison-038/p1-base-019-4-comparison.json` |
| `std output` | 1 | `semantic-comparison-037/p1-base-020-2-comparison.json` |
| `subcomponents` | 2 | `semantic-comparison-038/p1-base-020-4-comparison.json` |
| `suites` | 1 | `semantic-comparison-038/p1-base-020-7-comparison.json` |
| `tb` | 1 | `semantic-comparison-039/p1-base-021-1-comparison.json` |
| `unencodable` | 2 | `semantic-comparison-039/p1-base-022-5-comparison.json` |
| `unfocused` | 1 | `semantic-comparison-039/p1-base-022-5-comparison.json` |
| `wakes` | 1 | `semantic-comparison-041/p1-base-024-2-comparison.json` |
| `wch` | 1 | `semantic-comparison-041/p1-base-024-3-comparison.json` |
| `wrlock` | 1 | `semantic-comparison-041/p1-base-024-5-comparison.json` |

## 5. 부모 처분 기록은 있으나 합의 조치 일부가 반영되지 않은 단어

부모가 일부만 골랐을 가능성이 있다. 의도적 미선택인지는 단어마다 다시 확인하지 않았다.

| 단어 | 미반영 합의 조치 수 | 비교 파일 |
| --- | ---: | --- |
| `acceptor` | 4 | `semantic-comparison-001/p1-base-001-comparison.json` |
| `attaching` | 2 | `semantic-comparison-001/p1-base-001-comparison.json` |
| `time windows` | 4 | `semantic-comparison-040/p1-base-021-3-comparison.json` |
| `timeouts` | 109 | `semantic-comparison-040/p1-base-021-3-comparison.json` |
