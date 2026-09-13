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
  function staticName(node) {
    return node && (ts.isIdentifier(node) || ts.isPrivateIdentifier(node) || ts.isStringLiteral(node))
      ? node : null;
  }
  function bindingName(node) {
    const parent = node.parent;
    if ((ts.isVariableDeclaration(parent) || ts.isPropertyAssignment(parent) || ts.isPropertyDeclaration(parent))
      && parent.initializer === node) return staticName(parent.name);
    if (ts.isBinaryExpression(parent) && parent.operatorToken.kind === ts.SyntaxKind.EqualsToken && parent.right === node) {
      return staticName(ts.isPropertyAccessExpression(parent.left) ? parent.left.name : parent.left);
    }
    return null;
  }
  function visit(node, functionDepth = 0) {
    const isNamedFunction = ts.isFunctionDeclaration(node) || ts.isMethodDeclaration(node)
      || ts.isMethodSignature(node) || ts.isConstructorDeclaration(node)
      || ts.isGetAccessorDeclaration(node) || ts.isSetAccessorDeclaration(node);
    const boundName = ts.isArrowFunction(node) || ts.isFunctionExpression(node) ? bindingName(node) : null;
    if ((isNamedFunction || boundName) && functionDepth === 0) {
      const isConstructor = ts.isConstructorDeclaration(node);
      const name = isConstructor ? 'constructor'
        : boundName ? boundName.getText(source) : staticName(node.name)?.getText(source);
      if (name) declarations.push({ name, kind: 'fn', start: line(node.getStart(source)),
        name_line: line((boundName || node.name || node).getStart(source)), end: line(node.getEnd() - 1) });
    }
    if (ts.isCallExpression(node) || ts.isNewExpression(node)) {
      calls.push({ function: node.expression.getText(source), line: line(node.getStart(source)), end: line(node.getEnd() - 1) });
    }
    // File overview lists named declarations outside other named callables.
    // An anonymous registration callback is not itself a named declaration.
    const nextDepth = functionDepth + Number(isNamedFunction || Boolean(boundName));
    ts.forEachChild(node, child => visit(child, nextDepth));
  }
  visit(source);
  process.stdout.write(JSON.stringify({ oracle: 'TypeScript compiler', version: ts.version,
    file_sha256: crypto.createHash('sha256').update(bytes).digest('hex'), declarations, calls }));
}
