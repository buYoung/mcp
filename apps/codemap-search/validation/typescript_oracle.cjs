// Independent compiler AST positions. This never evaluates the inspected source.
const fs = require('node:fs');
const crypto = require('node:crypto');
const ts = require('typescript');

const path = process.argv[2];
const bytes = fs.readFileSync(path);
const source = ts.createSourceFile(path, bytes.toString('utf8'), ts.ScriptTarget.Latest, true);
if (source.parseDiagnostics.length) {
  process.stdout.write(JSON.stringify({ oracle: 'TypeScript compiler', version: ts.version,
    diagnostics: source.parseDiagnostics.map(diagnostic => ({ code: diagnostic.code,
      message: ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n'), start: diagnostic.start })) }));
  process.exitCode = 1;
} else {
  const declarations = [];
  const calls = [];
  const line = position => source.getLineAndCharacterOfPosition(position).line + 1;
  function visit(node, functionDepth = 0) {
    const isNamedFunction = ts.isFunctionDeclaration(node) || ts.isMethodDeclaration(node)
      || ts.isMethodSignature(node) || ts.isConstructorDeclaration(node)
      || ts.isGetAccessorDeclaration(node) || ts.isSetAccessorDeclaration(node);
    if (isNamedFunction && functionDepth === 0) {
      const isConstructor = ts.isConstructorDeclaration(node);
      const name = isConstructor ? 'constructor'
        : node.name && (ts.isIdentifier(node.name) || ts.isPrivateIdentifier(node.name) || ts.isStringLiteral(node.name))
          ? node.name.getText(source) : null;
      if (name) declarations.push({ name, kind: 'fn', start: line(node.getStart(source)),
        name_line: line(node.name ? node.name.getStart(source) : node.getStart(source)), end: line(node.getEnd() - 1) });
    }
    if (ts.isCallExpression(node) || ts.isNewExpression(node)) {
      calls.push({ function: node.expression.getText(source), line: line(node.getStart(source)), end: line(node.getEnd() - 1) });
    }
    const nextDepth = functionDepth + Number(isNamedFunction || ts.isArrowFunction(node) || ts.isFunctionExpression(node));
    ts.forEachChild(node, child => visit(child, nextDepth));
  }
  visit(source);
  process.stdout.write(JSON.stringify({ oracle: 'TypeScript compiler', version: ts.version,
    file_sha256: crypto.createHash('sha256').update(bytes).digest('hex'), declarations, calls }));
}
