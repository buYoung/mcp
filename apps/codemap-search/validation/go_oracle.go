// Go's standard parser is an independent oracle for declaration ranges and call syntax.
// It does not resolve receiver types, build tags, or imported package definitions.
package main

import (
	"encoding/json"
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"os"
	"strconv"
)

type declaration struct {
	Name  string `json:"name"`
	Kind  string `json:"kind"`
	Start int    `json:"start"`
	End   int    `json:"end"`
}

type call struct {
	Text string `json:"text"`
	Line int    `json:"line"`
}

func main() {
	if len(os.Args) != 2 {
		fmt.Fprintln(os.Stderr, "usage: go_oracle FILE")
		os.Exit(2)
	}
	source, err := os.ReadFile(os.Args[1])
	if err != nil {
		panic(err)
	}
	set := token.NewFileSet()
	file, err := parser.ParseFile(set, os.Args[1], source, parser.AllErrors)
	if err != nil {
		panic(err)
	}
	declarations := []declaration{}
	calls := []call{}
	imports := map[string]string{}
	for _, item := range file.Imports {
		name := ""
		if item.Name != nil {
			name = item.Name.Name
		}
		path, err := strconv.Unquote(item.Path.Value)
		if err != nil {
			panic(err)
		}
		imports[path] = name
	}
	ast.Inspect(file, func(node ast.Node) bool {
		switch item := node.(type) {
		case *ast.FuncDecl:
			declarations = append(declarations, declaration{item.Name.Name, "fn", set.Position(item.Pos()).Line, set.Position(item.End() - 1).Line})
		case *ast.InterfaceType:
			// codemap-search exposes named interface methods as fn declarations too.
			for _, method := range item.Methods.List {
				if _, ok := method.Type.(*ast.FuncType); ok {
					for _, name := range method.Names {
						declarations = append(declarations, declaration{name.Name, "fn", set.Position(method.Pos()).Line, set.Position(method.End() - 1).Line})
					}
				}
			}
		case *ast.CallExpr:
			start, end := set.Position(item.Fun.Pos()).Offset, set.Position(item.Fun.End()).Offset
			calls = append(calls, call{string(source[start:end]), set.Position(item.Pos()).Line})
		}
		return true
	})
	if err := json.NewEncoder(os.Stdout).Encode(map[string]any{"declarations": declarations, "calls": calls, "imports": imports}); err != nil {
		panic(err)
	}
}
