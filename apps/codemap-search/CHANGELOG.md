# Changelog

All notable changes to codemap-search are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.1.0] - 2026-11-28

### Improved

- A declaration and its implementation in different files are now linked when their parameters use types other than built-in ones. Before, they were linked only when every parameter type was a built-in such as `int` or `string`, so most interfaces showed `implementation: unresolved`. A type name now counts as one type in both files when both lead to the same declaration in the repository, both import it from the same module, or the language gives it one meaning everywhere, for example `String` in Java, `Int` in Kotlin, `std::string` in C++ and `object` in C#. A name the two files resolve differently stays unlinked, and so does one reached only through a wildcard import, a type alias or a C++ template. This applies to Go, TypeScript, Java, Kotlin, C#, C++, Dart, PHP, Rust, Scala and Swift. A relationship block that grows past its size limit ends with its truncation note, as before.
- C++ implementation relationships now reach across files. A virtual method lists the overrides in the files that include its header, and an override names the declaration it implements. Before, they appeared only when both classes were in one file, and not even then in most real code. Four things were in the way: a base class was looked up in the same file only; a file whose macros were expanded had no implementation relationships; every class inside an include guard counted as a conditional declaration; and a forward declaration (`class Name;`) or a declared constructor or destructor hid the class of that name. A base class in another file is found when the file includes its header directly and the name is visible from the namespace of the derived class. The included header is the file at the include path beside the including file; otherwise it is the only indexed file whose path ends with the include path. When several files end that way, the base class stays unlinked. Classes inside other `#if` blocks or on lines the preprocessor left out stay unlinked. Existing indexes are rebuilt once.
- `read` and `search` say that a file's event input is unavailable once per response instead of under every declaration of that file. This note appears for C and C++ files whose macros were expanded; reading one such file could repeat it more than a hundred times.
- A list of implementation candidates now says when it may be incomplete. Below the candidates of a declaration, `read` and `search` add one row, `not source-proven: N same-named method(s) in types naming <Type> as a base; the candidates above may be incomplete.`, when a type that names the declaration's type as a base has a method of the same name that could not be linked. Before, the list showed only the proven candidates and looked complete; an implementation in a generic class, under a base type that could not be resolved, or with a signature that could not be proven was left out without a sign. The row counts such methods and links none of them. It does not appear for Go, whose types name no base, or for Rust.
- Generic types now take part in call and implementation navigation in Java, Kotlin, C#, TypeScript, Dart and Swift. A call on a receiver whose declared type has type arguments, such as `codec.width()` on `Codec<String> codec`, is linked to the method of `Codec`, and a call to an abstract or overridden method shows `call declaration`. A generic class, and a class whose base has type arguments, are linked to the declarations they implement. Before, any type with type parameters and any reference with type arguments were set aside, so calls through such receivers stayed unresolved and their classes had no implementation relationships. A method whose parameter is written with a type parameter, such as `write(T value)` against `write(String value)`, is still not linked: the row `not source-proven` counts it. C++ templates and Rust are unchanged, because there the arguments select a specialization or an implementation block. Existing indexes are rebuilt once.
- A call on a receiver declared with a nullable type is now linked to the method of that type: `Tool?` in Kotlin, Swift, C# and Dart, `?Tool` in PHP, `Tool | null` and `Tool | undefined` in TypeScript, `Tool | None` and `Optional[Tool]` in Python. This covers the calls `tool?.size()`, `tool!!.size()` and `tool!.size()`. Before, the type was not read because of its null mark, so these calls stayed unresolved and their callers were listed as unconfirmed at best. A union of two types, such as `Gauge | Dial`, is still left unresolved. Existing indexes are rebuilt once.
- A call on a local variable with a written type is now linked to the method of that type in Kotlin and C#, such as `val picked: Store = ...` or `Store store = Open();`. In Kotlin this also holds for a property declared in the class body, such as `private val store: Store`. Before, these declarations gave no type, so the calls stayed unresolved. A local variable without a written type, such as `var store = Open();`, is still left unresolved.
- A first index build, and any refresh that rewrites at least as many files as the index holds, now ends with one search segment. Before, the number of segments left after a build depended on when the writer's buffers filled: the same repository ended with 2, 7 and 4 segments in three builds in a row. With seven segments `read` was about 2% slower than with one when four servers shared the host. A refresh of a few files adds a small segment as before.

### Changed

- Source files are parsed with tree-sitter 0.27 and newer grammars for C#, Dart, PowerShell, Scala, Swift and some other languages. Most files give the same declarations as before. Where macros or `#if` blocks split a C or C++ declaration, a declaration can now start on another line, and a few declarations are found or missed where they were not before. Some C# and Groovy files also show different local bindings and call scopes. In Swift, some top-level functions that were missed are found. In PowerShell, some calls get other ranges, and a few commands that were missed, such as `It "name" { ... }` in Pester tests, are found. Existing indexes are rebuilt once.
- C# files are parsed with tree-sitter-c-sharp 0.24.0, which reads more files without errors. godot's `InteropStructs.cs`, which was read as one error, now parses without errors and gives 281 declarations instead of 127. One form now makes a whole file one error: an `#if` block in the middle of an `&&` condition whose operand holds `typeof(...)`, followed by an operand that starts with `!`. Such a file loses most of its declarations. Of the 1,366 C# files checked, only Newtonsoft.Json's `JsonSerializerInternalReader.cs` has this form, and 20 of its 63 declarations remain. A call inside a dereferenced pointer cast, such as `AsPointer` in `*(T*)Unsafe.AsPointer(ref value)`, is also missed. The cause and the upstream fix are recorded in `docs/csharp-grammar-0.24.ko.md`.

### Fixed

- In Swift, a call right after a prefix or binary operator keeps its own callee. `!flags.contains(x)` names `flags` instead of `!flags` as the receiver of `contains`, and a call after a binary operator, such as `Double` in `total / Double(count)`, is found. Before, the parser's grouping put the operator into the callee or its receiver. An implicit member call such as `.init(x)` is still not recorded. Existing indexes are rebuilt once.
- In Dart, a call in the body of a `=>` function is found with its own receiver. Before, the parser read `() => Notifier(1)` as a call of the function `() => Notifier`, so the call to `Notifier` was missing, and `(e) => e.toString()` named `(e) => e` as the receiver of `toString`. `Source routes` now treat such a body like a Java lambda, so a few routes that came from the earlier reading are gone. A `typedef` of a function type, such as `typedef Handler = Future<void> Function(Request);`, is listed as a type instead of a field, and its `Function(...)` is no longer counted as a call. Existing indexes are rebuilt once.
- A `static` method in Dart is no longer listed as an implementation of a same-named static method of a supertype. Existing indexes are rebuilt once.
- A Kotlin file that ends with an annotation and no line break after it no longer stops the server. Before, parsing such a file never finished: reading it got no answer, later requests got no answer either, and indexing the repository kept a processor core busy without end.
- A Kotlin class whose primary constructor starts on the line after the class name is read as one class again in two cases: when the class is nested in another class, and when annotations and a visibility modifier come before `constructor`. Before, the class ended at its name, its methods appeared as functions of the enclosing class or not at all, and declarations after it could be missing. Existing indexes are rebuilt once.
- In C#, PHP, Ruby and Lua, a method, class or namespace is kept when the parser could not read a statement inside its body. Before, one such statement removed the method that holds it and every class and namespace around that method; in C# this happened when `#if` split a statement, for example an `else` branch inside `#if`. A declaration whose name, parameters or base list could not be read is still left out. Existing indexes are rebuilt once.
- A call whose source names another type is no longer listed under `unconfirmed callers`. This list holds the calls a name's only indexed definition could not be confirmed for, and it also took calls that the source shows go elsewhere. Two kinds are now left out. One is a call through a receiver declared with a type this repository does not declare, when the definition is a function outside any type: Go `value.Load()` on an `*atomic.Int64` beside a package-level `Load`. The other is a call whose `::` path names such a type: `String::from` beside the only indexed `from` in Rust, `std::max` in C++, `\DateTimeImmutable::createFromFormat` in PHP. These calls are counted in the note that points to grep instead. A path through a type parameter, `Self` or the trait the definition implements can still reach it and stays listed. Kotlin parameter types are now read for this check.
- A call to an extension method through a value of a type this repository does not declare is listed under `unconfirmed callers` again. Before, a C# call such as `text.Shouted()` on a `string` was counted as a call to another type and only the grep note mentioned it.
- Calls to members of a C# `partial` class are connected. This covers a call to a method declared in another part of the class and a call through a field or parameter whose type is a partial class. Before, each part counted as a separate type of the same name, so the type could not be singled out: the call was left unresolved, or listed under `unconfirmed callers`. The indexed parts of one project now form one type: parts below the same `.csproj` file, or in the same folder when no project file is indexed. Parts below a folder that holds several `.csproj` files are left apart, because each project chooses its own sources. A part inside `#if` is left apart, and a part that is not indexed, such as a generated one, stays unknown. Existing indexes are rebuilt once.
- A method that is visible only inside its package or library is no longer linked to a same-named method outside it. This covers a Java method without an access modifier, which only a class of the same package can override, and a Dart method whose name starts with an underscore, which only the same library can override. Before, a subclass in another package or library that declared the same name and parameters was listed as an implementation. Existing indexes are rebuilt once.
- A Python call by a bare name is linked through an import only where that import binds the name. An import inside a function binds the name for that function alone. Before, a call of the same name in another function of the file was linked to the imported function too, although Python raises `NameError` there. A module alias such as `import pkg.mod as m` follows the same rule. A name that the module imports at its top level, after every `from ... import *` line, is now linked, because the star import cannot rebind it. Before, one star import anywhere in the file left every call of an imported name unlinked.
- A call is no longer linked to a method of the receiver's type when a parent type declares another overload with the same name and parameter count but other parameter types. Java, Kotlin, Scala, Groovy and Swift choose between the two by the argument types, which are not known here, so the call stays unlinked. Before, the method of the receiver's own type was always chosen: `parcel.seal("x")` was linked to `Parcel.seal(Object)`, although Java runs the inherited `Envelope.seal(String)`.
- The notice for a C, C++ or assembly file whose macros could not be expanded now names files by their path from the workspace root. Before, it held absolute paths, so the output showed where the workspace is, and the length of that path changed how much relationship context fitted into the same output: the same file could show a `[Partial source analysis ...]` notice in one workspace location and leave it out in another. Existing indexes are rebuilt once.
- In C, C++, Java and Groovy, a call that gives a local variable its first value, as in `int total = sum();`, is now linked to the function it calls. The check for a parameter of the same name read the declaration of that variable as a parameter list, found the name of the call in it, and left the call unlinked. For the same reason a recursive call in C and C++ was left unlinked.
- A call whose target is found through the declared type of its receiver is now resolved inside an `if` statement too, both in its condition and in its body. The check that sets aside declarations made under a condition, such as a class inside `#if`, was also applied to calls, and it counted a runtime `if` as such a condition. Every call of this kind inside an `if` was therefore left unresolved, although a call names the same declaration wherever it stands. Rust was not affected. Existing indexes are rebuilt once.
- In C#, a call inside a generic type no longer goes to the type of the same name that has no type parameters. With `Outcome` and `Outcome<T>` both declared, `this.Code()` inside `Outcome<T>` was linked to `Outcome.Code`. The number of type arguments now tells the two apart.
- A Dart class whose base has type arguments, such as `class TextCodec extends Codec<String>`, is no longer left out of implementation navigation. The type arguments were read as a base that could not be understood, so the whole class was set aside, including its calls through typed receivers.
- A Dart call written with `?.`, such as `gauge?.level()`, is recorded as a call to `level`. It was recorded as a call to the receiver, `gauge`, so the method never listed this caller.
- A Kotlin method with a nullable parameter, such as `fun accept(item: String?)`, is linked to the declaration it implements. Its signature was read as unknown, so it was linked to none.
- `Connected locations` at the end of a compact search page no longer lists the heading of a method without a row that says what it is connected to. This happened when the space for relations ended right after the heading. A relation row now also comes with its heading when the page already shows the location of that heading; alone, the row did not say which declaration it belongs to. The heading of an abstract declaration is still listed on its own.

## [1.0.0] - 2026-11-17

### Added

- `search` can use another repository's index. Pass a folder outside the start folder, as an absolute path or one relative to the start folder, or a linked worktree inside it, as `workspace_scope`; `search` then searches the index of the repository holding that folder and narrows the results to the folder. The first such call creates that index with the settings a server started in the repository would use, except `[filesystem_permissions]`, which stay this session's; the repository's settings files are only read. Results from that index show absolute paths, and once it exists, `read` and `grep` of files there take declarations and callers from it. The session's `read` policy must allow the whole repository folder. One such index stays open at a time; it closes when a request goes to another repository, after 10 minutes without use, or when its folder is removed. An index written in another format is reported as `Index unavailable:` and left unchanged. Output for the start folder is unchanged. Prompt-text change (tool descriptions / server instructions / initial instructions): +117 / 0 / 0 characters in a monorepo and 0 / 0 / 0 elsewhere.
- `read view=source` adds a trailer, `[Other files importing this window's declarations (static analysis, approximate)]`, that lists the other files importing the window's exported values, classes and constants, with the functions or lines that use them. The caller trailer finds functions by name, but these declarations reach their users through imports. Each supported programming language follows its own import rule: script modules with their re-exports, renamed exports and the defaults and namespaces they pass on (`export { default as Name } from`, `export * as name from`), Python modules, Go packages, Rust paths, JVM packages, C# and PHP namespaces, Swift modules, Dart libraries, Ruby and PowerShell relative loads, Lua modules, C and C++ includes, assembly `.include`, `#include` and `%include`, shell `source`, Nix `import` and Starlark `load`. Files in the same package, namespace or module count when they use the name. A default import, and a Vue, Svelte or Astro component import, is listed under the imported file's name at its first line. Files in nearer directories come first; each trailer stays within 1,536 bytes and 100 ms, and repeated reads fold it like the caller trailer. A lookup that reaches its time limit before finding any importing file says so instead of showing nothing. Each name is listed once, however often it is assigned, and a use inside a function that binds the same name itself, as a local, a Python or PowerShell assignment or parameter, or a shell `local`, is not counted. Shell names that hold hyphens, as Zsh widget functions do, are listed too. Setting `output.read.is_outside_callers_enabled = false` turns it off as well. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.

### Improved

- A git worktree without an index now starts from the index of another worktree of the same repository that has the current format, the one updated most recently, and leaves that index unchanged. The first pass re-extracts only the files whose content differs, plus native files, whose recorded macro-expansion inputs name the other worktree's files, before anything from the copy is shown. A worktree that already has an index is never copied into. When a file's modification time changes but its content does not, as in a copied index or after a checkout that rewrites identical files, the index records the new time in `content-mtimes.json`, so later passes, server starts and caller scans treat the file as current without reading it again, and `analyze` reports it as unchanged; such files are compared on all processor cores. On a 19,757-file repository under a machine load of 26–50, a new worktree differing from its neighbor in 42 files re-extracted 42 files instead of 19,760 and was ready in a median of 22.5 s instead of 32.2 s, and one at its neighbor's commit in 24.6 s instead of 35.2 s (three alternating runs each); a restart read no file again. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- On a large repository, default `search` finishes more than three times sooner and `search` with `caller_context: false` more than four times sooner, with the same output. The caller scan reads and classifies files on all processor cores and still applies each name's limit in the same file order. Caller resolution visits only the files whose recorded calls or imports use the symbol's names, and loads their sources on all processor cores first. Event-input diagnostics and implementation relations check whether a file has a recorded problem or proof before checking whether the file is visible to the workspace walk. On a 19,757-file repository the median default `search` takes 0.79–0.96 s instead of 3.07–3.49 s and the slowest 2.9–3.4 s instead of 13.2–15.9 s; with `caller_context: false` the median takes 58–68 ms instead of 428–518 ms. On a 1,494-file repository the median default `search` takes 517–570 ms instead of 792–816 ms. `read` median and 95th-percentile times stay within 3% of before. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- A fresh index of a large repository is ready more than three times sooner. Files are read and extracted on all processor cores and still added to the index in the same order. Extraction finds the enclosing nodes of a syntax node in one pass down the tree instead of one pass for each step up, and visits child nodes in order instead of looking each one up by position. A publication decodes stored documents on all processor cores, and module lookups resolve the links of each path once. Generating the initial config reads each directory's ignore files once instead of once for every directory below it. On a 19,757-file repository a fresh index is ready after 27.7–30.8 s instead of 95.5–96.1 s, and on a 1,494-file repository after 20.9 s instead of 29.9 s. Extracted documents and tool outputs are unchanged, and `search` and `read` median and 95th-percentile times stay within 4% of before. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- Indexes are less than half their former size. Stored documents are compressed with zstd and no longer keep copies of event sources, flow summaries, symbol names or docstrings; all of these stay searchable or are read from the files. Paths, modification times and content hashes are also kept in fast fields, so a refresh compares files and a publication finds documents moved by a merge without reading stored documents. A file whose modification time changed while its content did not, as after a touch, is no longer re-extracted. Event sources are read from the files when a generation is published and are used only while they still match the indexed text. On a 19,757-file repository a fresh index takes 161 MB instead of 429 MB, and on a 1,494-file repository 18 MB instead of 42 MB. `search` and `read` median and 95th-percentile times stay within 5% of before; `search` with `caller_context: false` is 4–9% faster. Indexes rebuild once. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- Publishing a new index generation now decodes only the changed documents and shares unchanged files with the previous generation, and the first publication after startup reuses the documents read at startup instead of decoding them again. Event-route analysis reuses its cache when an edit changes neither an event source nor a Rust file, and its passes share the function summaries that did not change. On a 19,757-file repository, peak memory at ready fell from 2.13–2.36 GB to 1.72–1.88 GB, and after editing 200 files from 3.38–3.65 GB to 2.22–2.63 GB. On a 1,494-file repository, peak memory at ready fell from 2.55 GB to 1.73 GB. Tool outputs are unchanged. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- The `read` description and initial guidance now name the importer trailer beside the caller trailer, call both static uses, and ask agents to follow the ones the question needs instead of every listed caller. Prompt-text change (tool descriptions / server instructions / initial instructions): +48 / 0 / +89 characters.
- Initial guidance now asks agents to search or grep the requested concepts repo-wide before narrowing, verify the ends, transfer or registration and conditions of requested links in source, and follow missing links before stopping or state the blocker and gap. The repo-wide discovery sentence moves from the monorepo-only guidance into the shared guidance. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / +178 characters, or +72 in monorepo workspaces.
- Search lists files outside the ranked results when one of their docstrings or string literals states a quoted query phrase or two adjacent query words, under `Exact phrase matches outside the ranking`. These rows use only tail rows and output bytes the ranked results leave unused, so ranked files, relations and source excerpts are unchanged. Single identifiers and queries with search syntax add none. Prompt-text change: 0 / 0 / 0 characters.
- Search results share repeated deferred-body notes and callee headers after result selection, preserving source ranges and output budgets. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- `initial_instructions` returns the scope inventory without collecting or displaying statistics. Static prompt-text change: 0 / 0 / 0 characters; the statistics block is removed from the initial result.
- Server instructions and the `search`, `grep` and `read` descriptions now ask agents to keep located notes of their findings and of each distinct relevant behaviour in returned source, to record each requested link with both ends, what passes or matches and any wiring or enabling condition, and to read missing ends. For an unresolved requested link, agents try one lookup past an intermediary for the underlying component's direct users, or past local registration for where the component is installed, enabled or configured. Agents read the enclosing logic when a snippet does not establish the behaviour, and before answering they organize findings by requested part, accounting for each note with a cited finding or an explicit gap. Prompt-text change (tool descriptions / server instructions / initial instructions): +985 / +517 / 0 characters.
- With Jev enabled, `read` and `grep` also judge the rows of a function that a read window cuts and runs of returned rows outside functions, grouped by the declaration that holds them. Before, only complete function bodies were judged and the rest always stayed. Adjacent omitted units of one file share one `_omitted lines:` note that names at most three of them. In `search`, an omitted body's outline rows and relation notes go with it, except a row that still holds a retained member, and a file whose displayed bodies are all omitted keeps only its heading and omission notes. When a client lowers the output cap, the room that omissions free is no longer refilled with deferred bodies, and bodies Jev protects are deferred last instead of never. On three code-navigation questions about a large repository, each run three times with `gpt-6-astra` (medium effort), sessions with TypeSafe Jev fulfilled 90 of 108 graded facts and sessions with `openai/gpt-6-luna-decisions` 85, against 90 with Jev off; their main-agent tokens were 1.008× and 0.833× those of the Jev-off sessions. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- Reading an omission note's exact location, or a read window inside one function, now returns that source without a new Jev judgment. Before, a reread was judged again and could return the same note. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- With Jev enabled, `search` also judges the functions around its first follow-up call candidates, at most eight per search, against the task. Unrelated ones are left out, related ones are marked `(Jev: related)` and listed first, and a failed judgment leaves the list as before. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- Jev requests now use the layout each decision model reads best. TypeSafe Jev, including `typesafe/*` through OpenRouter, gets one plain-text state with the task and a group of candidates, each under its question ID. OpenRouter `openai/*` models get one request per candidate, so OpenAI refusing one question no longer fails the questions about other candidates. Other OpenRouter decision models keep the JSON layout. All requests of one call run together under the call's deadline. Replaying 18 recorded sessions from a large repository with `openai/gpt-6-luna-decisions`, failed requests fell from 33 of 477 to 5 of 5,710 and stages that fell back to the ordinary output from 33 to 6. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- `output.jev.max_in_flight_requests` now accepts 1 to 8 and defaults to 8 (was at most and by default 3), and `output.jev.request_spacing_ms` accepts 100 or more and defaults to 100 (was at least and by default 300). Existing explicit values stay valid. With one request per candidate, 600 requests to `openai/gpt-6-luna-decisions` took 63 s without errors at 8 and 100 ms, against 185 s and two HTTP 503 responses at 3 and 300 ms; 600 single-candidate requests to TypeSafe took 64 s, all successful. On three code-navigation questions about a large repository, each run three times, the main agent's `wait` calls in sessions using `openai/gpt-6-luna-decisions` fell from 36 to 0. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- With a Jev filter enabled, `search`, `read` and `grep` keep their ordinary descriptions and add one sentence about omission notes, instead of switching to a separate Jev description. Prompt-text change (tool descriptions / server instructions / initial instructions) with Jev enabled: +908 (`grep` +498, `read` +247, `search` +163) / 0 / 0 characters; 0 / 0 / 0 with Jev off.
- `grep` with `view=source_grouped` no longer starts with `Locations refer to the enclosing file; 42:match and 43-context preserve the original grep rows.`; the input schema already defines these markers. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- Server instructions now ask agents to grep a shared literal, constant, route or field name to follow a link across languages or between client and server code, to follow delegation into shared code and check alternative paths such as legacy or flag-gated branches, and, when a request term can name several code concepts, to prefer the one the codebase names as its own feature or module. On three code-navigation questions about a large repository, each run five times with Jev off, sessions with and without this guidance each fulfilled 148 of 180 graded facts, and sessions with it used 1.010× the main-agent tokens. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / +606 / 0 characters.
- Server instructions now ask agents to cite each location in the answer as `path:start-end` covering the lines that establish it, or as `path:line` when one line does. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / +123 / 0 characters.
- Server instructions now ask agents not to read or cite test files or generated files unless the request is about tests or generated code, and to trace and cite the hand-written implementation instead. On three code-navigation questions about a large repository, each run three times per model with Jev off, GPT-6.1 Sol sessions with this guidance fulfilled 99 of 108 graded facts against 98 without it and used 0.941× the main-agent tokens; Claude Opus 5.5 sessions fulfilled 77 against 81 and used 0.945×. Answers citing a test or generated file fell from 5 to 0 of 9 with GPT-6.1 Sol and from 6 to 5 of 9 with Claude Opus 5.5. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / +160 / 0 characters.

### Removed

- **Breaking:** The `overview` tool is removed. Use `find` for directory listings, `read` with `view=definitions` for a file's declaration outline, and `initial_instructions` for the monorepo scope inventory. `search` no longer keeps a scope chosen by an earlier call; pass `workspace_scope` on every call that should stay in one scope. Config schema v33 removes `[output.overview]`, `tool_output.is_overview_stats_enabled` and `output.jev.overview_filter_enabled`, and drops `"overview"` from `output.jev.scope`. If automatic config updates are off, `"overview"` in `output.jev.scope` is ignored and the other listed tools stay selected. Prompt-text change: −1,374 (the `overview` description and parameters −1,336, `search` −26, `grep` −12) / +27 / −53 characters, or −64 in monorepo workspaces.
- **Breaking:** The `register_task` tool is removed. While a Jev filter is enabled, `initial_instructions` takes the user's complete request as a required `task` argument, and the server asks every judged unit one fixed yes/no question, so agents no longer write `task_query`, `questions` or `match`. Calling `register_task` answers `Tool not found`. `output.jev.search_filter_min_unrelated_probability` now applies to that one question. Prompt-text change (tool descriptions / server instructions / initial instructions) with Jev enabled: −2,523 (`register_task` and its parameters −2,628, the `task` parameter +105) / 0 / +184 characters; 0 / 0 / 0 with Jev off.
- Jev sends only each judged unit's own source, with its path, kind and name. Caller and callee source, relationship summaries and event evidence are no longer sent with a unit. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.

### Fixed

- Indexing the start folder now skips git linked worktrees checked out inside it. Their files are another checkout of the same repository and appeared in search results twice. A directory is a linked worktree when its `.git` file names a gitdir in the `worktrees` directory of a git common directory; submodules and nested repositories stay indexed. The index walk, file watching, the caller scan and the auto-mode file count all skip these directories, and an existing index drops their files at the next startup without a rebuild. `grep`, `find` and path suggestions still list the worktree's files; their matches note that the file is not in the index instead of showing its declarations. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- Index passes now let the segment merges they start finish before releasing the index writer, and rewrite segments in which more than 10% of the documents are deleted. Before, each pass cancelled its merges, so reindexed files kept their old copies and indexes kept growing (one index had 43 segments with 31% of its documents deleted). After editing and restoring 200 files three times, a scratch buyong-mcp index now stays at one segment and 42 MB, where it had 5 segments, 49 MB and 199 deleted documents. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- TypeScript and JavaScript files now record another module's default or namespace they pass on, `export { default as Name } from` and `export * as name from`, beside their renamed exports. Indexes rebuild once. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- Assembly files now record the preprocessor's `#include` and NASM's `%include` as imports, as they did `.include`. Indexes rebuild once. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- Bash function declarations now span their bodies, as Zsh ones do, so declaration ranges and the enclosing-function labels of trailers cover the whole function. Indexes rebuild once. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- A Groovy or Scala class, object or value counts as exported unless its own modifiers make it private or protected. A private member in its body, or a private Scala constructor, no longer makes declaration listings label it `not exported` or keeps it out of the importer trailer. Indexes rebuild once. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- Bash, Zsh and PowerShell scripts that source another script through a computed directory, as in `source "$(dirname "$0")/lib.sh"`, `source ${0:A:h}/lib.zsh` or `. $PSScriptRoot\lib.ps1`, now record that import. The literal rest of the path names the file, beside the sourcing script or as the end of one indexed path. Indexes rebuild once. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- A script import of a `.vue`, `.svelte` or `.astro` file now resolves to that component file, so caller resolution follows calls into the named exports of its script. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- A TypeScript type-only import specifier, `import { type Name }`, now records `Name` as the imported declaration instead of a binding called `type`, so caller resolution follows it. Indexes rebuild once. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- The caller trailer of `read view=source` no longer stops at its time limit on most reads of large workspaces. While the filesystem watcher keeps the index current, the lookup reads only indexed files whose recorded calls or imports use a looked-up name instead of walking the workspace, and caller resolution reuses the indexed extraction of a file whose content still matches the index instead of parsing it again. The listed callers are the same; they now appear in path and line order. Prompt-text change (tool descriptions / server instructions / initial instructions): 0 / 0 / 0 characters.
- Exact `event_key` searches with no hits explain the lookup scope and how to try ranked search, without silently falling back to it. Prompt-text change: −3 / 0 / 0 characters.
- `grep` file and count modes accept `view=source` and `view=source_grouped` as presentation-only options, while still rejecting content-only controls. Prompt-text change: 0 / 0 / 0 characters.
- `read` with `view=definitions` no longer reports indexed Sass files as an unsupported format. Prompt-text change: 0 / 0 / 0 characters.

## [0.14.2] - 2026-10-29

### Added

- Each tool now advertises `output.client.pi_max_bytes` as `_meta["pi/maxOutputBytes"]` in `tools/list`, so [@buyong/pi-mcp-adapter](https://www.npmjs.com/package/@buyong/pi-mcp-adapter) applies codemap-search's delivery limit without a matching `mcp.json` setting. pi's built-in MCP support ignores tool metadata and still cuts results at 20 KB.

## [0.14.1] - 2026-10-28

### Improved

- Server instructions now ask agents to send independent tool calls together in one response as separate calls and to begin with several narrow lookups instead of one broad search. The paragraph appears twice on purpose.

## [0.14.0] - 2026-10-26

### Added

- Added `read ranges` for 1–20 known file windows in one MCP call, with single-read aliases, shared presentation options, per-item errors, connection-local deduplication and whole-item continuation under the existing byte budgets. Either `file_path` or `ranges` is required, but not both; this is checked at runtime without top-level schema combinators.
- Added OpenRouter decision-model support with provider-specific credentials and strict response validation.

### Changed

- Updated the default TypeSafe decision model to `jev-1.14.0`.

## [0.13.0] - 2026-10-23

### Added

- Added up to three workspace path suggestions when `read`, `grep`, or `find` targets a missing path.
- Added cross-file matches for string keys and uppercase constants used as call arguments in `read view=source`; indexes rebuild once.
- Added `auto`/`on`/`off` indexing modes; `auto` starts on demand and skips non-repository folders or workspaces over a configurable file-count limit.

### Improved

- Linked C/C++ header declarations to unique definitions and their callers, including calls through indirect includes.
- Resolved C++ member calls through `this`, typed variables, and class names, excluding overloads and inheritance.
- Limited C/C++ macro reprocessing to files whose inputs changed.
- Reduced analysis time during initial indexing of large repositories.
- Explained omitted caller summaries with index status, file status, or timeout details.
- Reduced repeated warnings and backed off failed background index updates to retry at intervals of up to 30 seconds.

### Fixed

- Fixed `initial_instructions` errors from older clients by ignoring legacy arguments.
- Fixed obsolete index files accumulating on disk after updates.
- Fixed missing caller summaries and false stale-result warnings while another process writes to the shared index.
- Restored caller information in C/C++ files with macro expansion enabled.
- Fixed unnecessary index updates from ignored file changes, while preserving updates needed for C/C++ macro expansion.
- Fixed nested repositories losing indexed files because of a parent repository's ignore rules.
- Fixed outdated generated config comments during upgrades while preserving their language and existing settings.
- Fixed search errors for queries such as `symbol:*` by falling back to plain-term search.

### Changed

- Changed default `read`, `grep`, and `find` access from `workspace` to `anywhere`, leaving existing settings unchanged.
- Hid `register_task` from the tool list when all Jev filters are disabled; direct calls still work.

## [0.12.0] - 2026-10-21

### Added

- Added an opt-in `synonyms` tool with a local dictionary that ranks meanings by how terms are used in your codebase.
- Added caller summaries beyond the displayed range in `read view=source`, with `[output.read].is_outside_callers_enabled` to turn them off.
- Added client-wide code navigation instructions management, with injected guidance in English.

### Improved

- Improved caller and callee detection for typed method calls and for functions imported from other files.
- Followed re-exports in TypeScript, JavaScript, Python, and Dart to resolve calls to their original declarations.
- Resolved JS/TS workspace package imports to source entry points, including `exports` subpaths.
- Limited caller summary lookups to 250ms per read, keeping callers found so far and noting when the limit is reached.
- Reduced data-file search output by showing counts for unmatched keys and limiting source text.

### Fixed

- Fixed long `else if` chains preventing an entire repository's index from updating.
- Fixed secret masking hiding code after operators and missing values assigned in short variable declarations.

### Changed

- Moved task registration from `initial_instructions` to the new `register_task` tool; `initial_instructions` now takes no arguments.
- Triggered a one-time automatic rebuild of existing indexes for expanded caller detection and codebase evidence.

## [0.11.0] - 2026-09-28

### Added

- Added Jev file recommendations and automatic filters for `search`, `read`, `grep`, and non-root overviews using registered task questions.
- Added repository and global Jev credential files with automatic reload and environment-variable fallback.
- Added output limits for pi and opencode, with source ranges for oversized searches and guidance to narrow other results.

### Improved

- Reduced repeated source text across tools, with an option to request the original text again.
- Preserved source bodies when Jev lacks enough context to judge relevance.
- Reduced processing overhead for large MCP inputs, sensitive-data masking, and source relationship analysis.

### Fixed

- Fixed tool execution errors being reported as JSON-RPC errors by returning error-marked tool results.
- Fixed Codex-specific output-budget guidance appearing for other clients.
- Fixed Windows build failures caused by incompatible build-tool versions.

### Changed

- Enabled client output limits by default and unified their default values at `100000`.
- Simplified Jev filtering settings to an on/off switch and a tool list, with automatic migration of existing settings.

### Removed

- Removed configurable environment-variable names for Jev authentication.

## [0.10.0] - 2026-09-21

### Added

- Added opt-in masking for 9 common PII types and 81 national identifier types across 18 countries.
- Added `fd` file discovery and per-root `tokei` code statistics.
- Added shared and per-tool output limits.
- Added Codex configuration output.
- Added CLI and MCP analytics for index status and read activity, with pagination and 30-day history with automatic cleanup.

### Improved

- Reduced repeated relationship analysis by reusing cached results and avoiding duplicate work across concurrent runs.
- Reduced release archive size from 33.4MiB to 3.4MiB.

### Fixed

- Fixed reindexing that could expand beyond caller-specified paths.
- Fixed index readiness and retries when multiple processes compete for the same index.
- Fixed search output budgeting to preserve later results and flag partial output.

### Changed

- Reorganized output, indexing, and exclusion settings while preserving values, inheritance, comments, and support for legacy keys.

## [0.9.1] - 2026-09-17

### Added

- Added custom field and regex masking rules, with exact-value exceptions.

### Security

- Masked sensitive values in MCP output while preserving variable references and safe literals.
- Stopped echoing search queries in empty results when masking context is unavailable.

`## [0.9.0] - 2026-09-17

### Added

- Added free-function scope, callee locations, and same-file constant initializers to `read` and `grep` context.
- Added automatic links between abstract declarations and their implementations.
- Added event relationship navigation and source-path tracing across 18 languages.
- Added conditional value-flow and error-return summaries.
- Added output controls and size-unit support.

### Improved

- Organized `read` and `grep` results by file, with clearer declaration sections and hierarchies.
- Expanded default `grep` context to include function bodies.
- Improved call lookup efficiency by avoiding unnecessary parsing and repeated analysis.

### Fixed

- Fixed recursive exclusion rules and exclusions for files passed directly to `grep`.
- Fixed false call links from strings, comments, cross-language matches, and object members.
- Fixed parsing errors in large repositories.
- Fixed `grep` modes `files_with_matches` and `count` returning unwanted symbol context.
- Fixed search output losing source text or showing relationships outside the displayed range.
- Fixed `cm find` command routing.
- Fixed inconsistent readiness checks during initial indexing.

### Changed

- Enabled event analysis and related context by default.
- Excluded test code from automatic context by default, with per-language rules that can be replaced or disabled.
- Consolidated exclusion settings under `exclude`, preserving values, comments, and support for legacy configuration keys.
- Raised default read output to 5MiB and search output to 1MiB, with larger caller/callee limits and relationship budgets.

## [0.8.1] - 2026-09-12

### Added

- Added per-project exclusion rules and configuration switching.

## [0.8.0] - 2026-09-11

### Added

- Added file members and caller/callee context under `# symbols` in `read` and `grep`, keeping source text under `# results`.

### Improved

- Improved scoped search by applying scope filters before collecting candidates.
- Improved search relevance for generated and translated files and implementation candidates.
- Clarified regex escaping and next steps for searches with no results.

### Fixed

- Fixed searches losing their subfolder scope.

## [0.7.0] - 2026-09-04

### Improved
- Improved `grep` no-result messages with active filters and next-step guidance.
- Clarified `output_mode` options and updated `search` and `grep` guidance for more efficient navigation.
- Improved `overview` and `read` handling of unsupported file types.

## [0.6.0] - 2026-08-28

### Added
- Added navigation for Vue, Astro, and Svelte components across markup, scripts, styles, and original source locations.
- Added C#, PHP, Ruby, Lua, Swift, Dart, Scala, Groovy/Gradle, PowerShell, and Nix language support.
- Added Bash, CMake, Containerfile, CSS, GraphQL, HCL, HTML, JSON, TOML, YAML, XML, Less, Dockerfile, and Makefile support.
- Added optional Markdown and MDX indexing for headings, links, code blocks, and full text via `is_document_support_enabled`.
- Added optional language groups for shell, infrastructure, interface, and build formats.

### Improved
- Improved search relevance by skipping text, lock, source map, minified, and bundle files by default.
- Improved caller/callee navigation for receiver calls, relative and dynamic imports, local shadowing, and indirection.
- Improved live indexing for file creation, changes, deletion, and runtime language-setting updates.
- Improved workspace safety by refusing to index a user's home directory as the workspace root.

### Fixed
- Fixed standalone SCSS files being indexed as a supported language.

### Changed
- Changed shell, infrastructure, interface, and build groups to opt-in; `find`, `grep`, `read`, and direct parsing remain available.
- Changed `EXTRACTION_FORMAT_VERSION` to `v18` for the expanded language and symbol data.

### Internal
- Replaced legacy benchmark data with a fixed Directus search-quality harness and 14 tasks.

## [0.5.0] - 2026-07-20

### Added
- Added static collection read/write tracking for TypeScript, JavaScript, Python, Java, Kotlin, Go, Rust, C, and C++.
- Added code relationship extraction for Vue, Astro, and Svelte components, plus SQL syntax support.

### Improved
- Improved monorepo navigation with workspace scopes and source roots included in `initial_instructions`.
- Improved workspace selection by rejecting duplicate or ambiguous monorepo paths.
- Improved relationship extraction for multilingual code searches.

### Internal
- Expanded benchmark fixtures, validation, attempt records, episode claims, terminal event checks, and cache reuse.

## [0.4.0] - 2026-07-01

### Added
- Added live `config.toml` watching so config changes sync without restarting.
- Added Korean and English `config.toml` comments selected from the OS locale.
- Added permission-aware filesystem tool descriptions that show the active access policy.

### Improved
- Improved config syncing with debounce handling for rapid `config.toml` edits.
- Improved filesystem permission docs with clearer policy and auto-sync guidance.

## [0.3.0] - 2026-06-30

### Added
- Added workspace-scoped MCP search so requests stay within the active workspace range.
- Added monorepo workspace detection for search and file exploration.
- Added `[update].config_auto_update` to control automatic `config.toml` creation and sync.

### Improved
- Improved file discovery with expanded workspace filters and better parent-child path handling.
- Improved search results and summaries by showing the active workspace scope.
- Improved `config.toml` docs for monorepo and workspace scope settings.

### Fixed
- Fixed repository config templates overriding global settings during config generation or sync.

### Internal
- Improved crates.io publishing reliability with retry and error handling.

## [0.2.0] - 2026-06-26

### Added
- Added Tree-sitter based caller and callee tracking for more precise code navigation.
- Added navigation context settings, including `navigation_context_default` and `navigation_callsite_budget`.

### Improved
- Improved search ranking with language and file extension hints.
- Improved definition and reference tagging through Tree-sitter `tags.scm` queries.
- Improved `codemap-search` README install guidance for crates.io, local checkout, and Claude Code user-scope setup.

## [0.1.6] - 2026-06-22

- No user-facing changes in this release.

## [0.1.5] - 2026-06-22

- No user-facing changes in this release.

## [0.1.4] - 2026-06-21

- No user-facing changes in this release.

## [0.1.3] - 2026-06-21

- No user-facing changes in this release.

## [0.1.2] - 2026-06-21

- No user-facing changes in this release.

## [0.1.1] - 2026-06-21

- No user-facing changes in this release.

## [0.1.0] - 2026-06-21

### Added
- Added `codemap-search`, a local MCP server and CLI for code navigation.
- Added BM25 search across symbols, docstrings, string literals, and error messages.
- Added `overview`, `search`, `read`, `grep`, and `find` tools for repository exploration.
- Added caller and callee context in detailed `search` results.
- Added automatic background indexing and file watching for code changes.
- Added repo-local `.codemap/config.toml` creation and schema migration.
- Added support for Rust, Python, TypeScript, JavaScript, Go, Java, Kotlin, C, C++, and Assembly.
- Added Linux, macOS, and Windows release artifacts with sha256 checksums.
- Added POSIX install script, Homebrew formula, WinGet manifests, and crates.io publishing.

### Improved
- Improved `overview` output with directory-focused summaries and folded tree formatting.
- Improved `search` output with line-numbered snippets and shorter high-signal result details.
- Improved `grep` and `find` glob handling with ripgrep-style include and exclude patterns.
- Improved `read` and `grep` inputs with aliases like `path`, `file`, `query`, `start_line`, and `end_line`.
- Improved cold indexing feedback with a visible `warming up` state.
- Improved Linux compatibility so GNU and musl builds support Ubuntu 22.04+.

### Fixed
- Fixed `serverInfo.version` so MCP clients see the Cargo package version.
- Fixed Windows release checksum formatting and shell variable expansion in release builds.
- Fixed benchmark-answer text in tool examples by replacing it with neutral sample paths.

### Changed
- Changed `get_codemap` to `overview` for a clearer tool name.
- Changed default `grep` output to include matching lines as `file:line:text`.
- Changed git ignore handling so `codemap-search` no longer edits `.git/info/exclude`.
- Changed `register_git_exclude` and `respect_git_exclude` into the unified `use_git_ignore` setting.

### Security
- Added repo-confined filesystem permission controls for `find`, `grep`, and `read`.
- Added stronger path boundary checks for absolute paths and workspace-relative input.
- Hardened the install script with atomic installs, required tool checks, and symlink rejection.

