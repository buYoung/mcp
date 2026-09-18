import { createRequire } from "node:module";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const ts = require("typescript");
const source = join(dirname(fileURLToPath(import.meta.url)), "merchant_service.ts");
const output = mkdtempSync(join(tmpdir(), "codemap-merchant-fixture-"));

try {
  const program = ts.createProgram([source], {
    strict: true,
    target: ts.ScriptTarget.ES2022,
    module: ts.ModuleKind.CommonJS,
    outDir: output,
    noEmitOnError: true,
  });
  const diagnostics = ts.getPreEmitDiagnostics(program);
  if (diagnostics.length > 0) {
    throw new Error(ts.formatDiagnosticsWithColorAndContext(diagnostics, {
      getCurrentDirectory: () => process.cwd(),
      getCanonicalFileName: name => name,
      getNewLine: () => "\n",
    }));
  }
  const emitted = program.emit();
  if (emitted.emitSkipped) {
    throw new Error("Fixture compilation did not produce an executable module");
  }
  const { runAllScenarios } = require(join(output, "merchant_service.js"));
  const results = await runAllScenarios();
  console.log(JSON.stringify({
    scenarios: results.length,
    merchants: results.reduce((sum, row) => sum + row.merchants, 0),
    documents: results.reduce((sum, row) => sum + row.documents, 0),
    checks: results.reduce((sum, row) => sum + row.checks, 0),
    deliveredEvents: results.reduce((sum, row) => sum + row.deliveredEvents, 0),
    results,
  }, null, 2));
} finally {
  rmSync(output, { recursive: true, force: true });
}
