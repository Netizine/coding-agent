// Development-only AST inventory. Never imports or executes reference packages.
package main

import (
	"encoding/json"
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

type File struct {
	Path             string   `json:"path"`
	Package          string   `json:"package"`
	Scope            string   `json:"scope"`
	BuildConstraints []string `json:"build_constraints"`
	Symbols          []string `json:"symbols"`
	Tests            []string `json:"tests"`
	Example          bool     `json:"example"`
}

func main() {
	if len(os.Args) != 3 {
		panic("usage: go run tools/go-inventory/main.go SOURCE OUTPUT")
	}
	root, err := filepath.Abs(os.Args[1])
	if err != nil {
		panic(err)
	}
	files := []File{}
	err = filepath.WalkDir(root, func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() || !strings.HasSuffix(path, ".go") {
			return nil
		}
		rel, err := filepath.Rel(root, path)
		if err != nil {
			return err
		}
		rel = filepath.ToSlash(rel)
		parsed, err := parser.ParseFile(token.NewFileSet(), path, nil, parser.ParseComments)
		if err != nil {
			return err
		}
		item := File{Path: rel, Package: parsed.Name.Name, Scope: "public", Symbols: []string{}, Tests: []string{}, BuildConstraints: []string{}, Example: strings.HasPrefix(rel, "examples/")}
		if strings.HasPrefix(rel, "internal/") || strings.Contains(rel, "/internal/") {
			item.Scope = "internal"
		}
		if strings.HasPrefix(rel, "_catalog/") || strings.HasPrefix(rel, "cmd/") {
			item.Scope = "tooling"
		}
		if item.Example {
			item.Scope = "example"
		}
		for _, group := range parsed.Comments {
			for _, c := range group.List {
				if strings.HasPrefix(c.Text, "//go:build") || strings.HasPrefix(c.Text, "// +build") {
					item.BuildConstraints = append(item.BuildConstraints, c.Text)
				}
			}
		}
		for _, decl := range parsed.Decls {
			switch decl := decl.(type) {
			case *ast.FuncDecl:
				name := decl.Name.Name
				if strings.HasSuffix(rel, "_test.go") {
					if decl.Recv == nil && strings.HasPrefix(name, "Test") {
						item.Tests = append(item.Tests, name)
					}
				} else if ast.IsExported(name) {
					if decl.Recv != nil {
						name = receiver(decl.Recv.List[0].Type) + "." + name
					}
					item.Symbols = append(item.Symbols, name)
				}
			case *ast.GenDecl:
				if strings.HasSuffix(rel, "_test.go") {
					continue
				}
				for _, spec := range decl.Specs {
					switch spec := spec.(type) {
					case *ast.TypeSpec:
						if !ast.IsExported(spec.Name.Name) {
							continue
						}
						item.Symbols = append(item.Symbols, spec.Name.Name)
						var fields *ast.FieldList
						switch t := spec.Type.(type) {
						case *ast.StructType:
							fields = t.Fields
						case *ast.InterfaceType:
							fields = t.Methods
						}
						if fields != nil {
							for _, field := range fields.List {
								for _, name := range field.Names {
									if ast.IsExported(name.Name) {
										item.Symbols = append(item.Symbols, spec.Name.Name+"."+name.Name)
									}
								}
							}
						}
					case *ast.ValueSpec:
						for _, name := range spec.Names {
							if ast.IsExported(name.Name) {
								item.Symbols = append(item.Symbols, name.Name)
							}
						}
					}
				}
			}
		}
		sort.Strings(item.Symbols)
		sort.Strings(item.Tests)
		files = append(files, item)
		return nil
	})
	if err != nil {
		panic(err)
	}
	sort.Slice(files, func(i, j int) bool { return files[i].Path < files[j].Path })
	out := map[string]any{"schema_version": 1, "method": "Go AST parse of all source/build variants; declarations only, no type checking, promoted-member resolution, package initialization or test execution", "files": files}
	data, err := json.MarshalIndent(out, "", "  ")
	if err != nil {
		panic(err)
	}
	if err := os.WriteFile(os.Args[2], append(data, '\n'), 0644); err != nil {
		panic(err)
	}
	fmt.Printf("Inventoried %d Go files\n", len(files))
}
func receiver(expr ast.Expr) string {
	switch expr := expr.(type) {
	case *ast.Ident:
		return expr.Name
	case *ast.StarExpr:
		return receiver(expr.X)
	case *ast.IndexExpr:
		return receiver(expr.X)
	case *ast.IndexListExpr:
		return receiver(expr.X)
	default:
		return "?"
	}
}
