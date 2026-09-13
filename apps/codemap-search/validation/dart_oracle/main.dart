import 'dart:convert';
import 'dart:io';
import 'package:analyzer/dart/analysis/utilities.dart';
import 'package:analyzer/dart/ast/ast.dart';

void main(List<String> arguments) {
  final path = arguments.single;
  final result = parseString(content: File(path).readAsStringSync(), path: path, throwIfDiagnostics: false);
  if (result.errors.isNotEmpty) {
    stderr.writeln(result.errors.join('\n'));
    exitCode = 1;
    return;
  }
  final declarations = <Map<String, Object>>[];
  void visit(AstNode node, int functionDepth) {
    String? name;
    int? nameOffset;
    if (node is FunctionDeclaration) {
      name = node.name.lexeme;
      nameOffset = node.name.offset;
    } else if (node is MethodDeclaration) {
      name = node.name.lexeme;
      nameOffset = node.name.offset;
    } else if (node is ConstructorDeclaration) {
      name = node.name?.lexeme ?? node.typeName?.name ?? 'new';
      nameOffset = node.name?.offset ?? node.typeName?.offset ?? node.offset;
    }
    if (name != null && functionDepth == 0) {
      declarations.add({'name': name, 'kind': 'fn',
        'start': result.lineInfo.getLocation(node.offset).lineNumber,
        'name_line': result.lineInfo.getLocation(nameOffset!).lineNumber,
        'end': result.lineInfo.getLocation(node.end - 1).lineNumber});
    }
    final nextDepth = functionDepth + (name != null || node is FunctionExpression ? 1 : 0);
    for (final child in node.childEntities.whereType<AstNode>()) {
      visit(child, nextDepth);
    }
  }
  visit(result.unit, 0);
  print(jsonEncode({'oracle': 'Dart analyzer parseString', 'version': '13.0.0', 'declarations': declarations,
    'scope': 'Named functions, methods and constructors outside function bodies; no type resolution'}));
}
