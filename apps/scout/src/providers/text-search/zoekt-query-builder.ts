export interface QueryBuildOptions {
    pattern: string;
    glob?: string | undefined;
    type?: string | undefined;
    caseInsensitive?: boolean | undefined;
    relativePathPrefix?: string | undefined;
}

/** Common `type` aliases mapped to zoekt `lang:` language names. */
const TYPE_TO_LANGUAGE: Record<string, string> = {
    ts: "TypeScript",
    tsx: "TypeScript",
    js: "JavaScript",
    jsx: "JavaScript",
    mjs: "JavaScript",
    cjs: "JavaScript",
    py: "Python",
    go: "Go",
    kt: "Kotlin",
    kts: "Kotlin",
    rs: "Rust",
    java: "Java",
    rb: "Ruby",
    c: "C",
    h: "C",
    cpp: "C++",
    cc: "C++",
    hpp: "C++",
    cs: "C#",
    php: "PHP",
    swift: "Swift",
    scala: "Scala",
    sh: "Shell",
    bash: "Shell",
    md: "Markdown",
    json: "JSON",
    yaml: "YAML",
    yml: "YAML",
    html: "HTML",
    css: "CSS",
    scss: "SCSS",
    sql: "SQL",
    toml: "TOML",
};

/**
 * Builds a zoekt query string from the Grep-style inputs. zoekt's `q` parameter
 * accepts a space-separated AND of atoms (DESIGN §6.1 실측): `lang:`/`file:`
 * filters plus the raw RE2 pattern.
 */
export function buildZoektQuery(options: QueryBuildOptions): string {
    const atoms: string[] = [];

    if (options.caseInsensitive === true) {
        atoms.push("case:no");
    }

    const normalizedType = options.type?.trim().toLowerCase();
    if (normalizedType != null && normalizedType.length > 0) {
        const language = TYPE_TO_LANGUAGE[normalizedType];
        if (language != null) {
            atoms.push(`lang:${language}`);
        } else {
            atoms.push(`file:\\.${escapeRegExpSource(normalizedType)}$`);
        }
    }

    const normalizedGlob = options.glob?.trim();
    if (normalizedGlob != null && normalizedGlob.length > 0) {
        atoms.push(`file:${globToRegExpSource(normalizedGlob)}`);
    }

    if (options.relativePathPrefix != null && options.relativePathPrefix.length > 0) {
        atoms.push(`file:^${escapeRegExpSource(options.relativePathPrefix)}/`);
    }

    atoms.push(escapeQueryWhitespace(options.pattern));
    return atoms.join(" ");
}

/**
 * zoekt's query parser splits atoms on whitespace, so a Grep-style phrase pattern
 * like `class Foo` would become an AND of two regexes. Escaping unescaped spaces
 * keeps it a single RE2 atom (`\ ` matches a literal space) while preserving regex
 * semantics — unlike double-quoting, which would force a literal substring.
 */
function escapeQueryWhitespace(pattern: string): string {
    return pattern.replace(/(?<!\\) /g, "\\ ");
}

function escapeRegExpSource(value: string): string {
    return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * Converts a small subset of glob syntax to an RE2 source for zoekt `file:`.
 * Supports `**`, `*`, `?`, `.`, and `{a,b}` alternation.
 */
function globToRegExpSource(glob: string): string {
    let source = "";
    for (let index = 0; index < glob.length; index += 1) {
        const character = glob[index] ?? "";
        if (character === "*") {
            if (glob[index + 1] === "*") {
                source += ".*";
                index += 1;
            } else {
                source += "[^/]*";
            }
        } else if (character === "?") {
            source += "[^/]";
        } else if (character === "{") {
            source += "(";
        } else if (character === "}") {
            source += ")";
        } else if (character === ",") {
            source += "|";
        } else if (".+^$()|[]\\".includes(character)) {
            source += `\\${character}`;
        } else {
            source += character;
        }
    }
    return source;
}
