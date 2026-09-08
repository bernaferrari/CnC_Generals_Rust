"""C++ function-definition candidates, including unpreprocessed branches.

This is syntax discovery, not proof that every macro-generated definition or
behavior has been reviewed. Parse errors remain visible and block automatic
symbol-ownership credit. Never fall back to counting calls with a regex.
Install requirements-provenance.txt in the Python environment running the tools.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from importlib.metadata import version


PARSER_ID = "tree-sitter-cpp/0.23.4;tree-sitter/0.25.2"


@dataclass
class CppDefinitionInventory:
    symbols: list[dict[str, object]]
    diagnostics: list[dict[str, object]]


def extract_cpp_definition_inventory(path: Path) -> CppDefinitionInventory:
    try:
        from tree_sitter import Language, Parser
        import tree_sitter_cpp
    except ImportError as error:
        raise RuntimeError(
            "C++ definition discovery requires the pinned parser packages. "
            "Install Code/Main/scripts/requirements-provenance.txt in a Python "
            "virtual environment and run the parity tools with that Python."
        ) from error

    installed = f"tree-sitter-cpp/{version('tree-sitter-cpp')};tree-sitter/{version('tree-sitter')}"
    if installed != PARSER_ID:
        raise RuntimeError(
            f"Expected pinned C++ parser {PARSER_ID}, found {installed}; install requirements-provenance.txt"
        )

    source = path.read_bytes()
    parser = Parser(Language(tree_sitter_cpp.language()))
    tree = parser.parse(source)
    symbols: list[dict[str, object]] = []
    diagnostics: list[dict[str, object]] = []

    def text(node) -> str:
        return source[node.start_byte : node.end_byte].decode("utf-8", errors="replace")

    def declarator_name(node) -> str | None:
        if node is None:
            return None
        if node.type in {
            "identifier",
            "field_identifier",
            "destructor_name",
            "operator_name",
        }:
            return " ".join(text(node).split())
        if node.type == "qualified_identifier":
            scope = node.child_by_field_name("scope")
            name = declarator_name(node.child_by_field_name("name"))
            return f"{text(scope) if scope else ''}::{name}" if name else None
        if node.type == "operator_cast":
            declarator = node.child_by_field_name("declarator")
            if declarator is not None:
                return " ".join(
                    source[node.start_byte : declarator.start_byte]
                    .decode("utf-8", errors="replace")
                    .split()
                )
        if node.type in {"reference_declarator", "parenthesized_declarator"}:
            children = node.named_children
            if len(children) == 1:
                return declarator_name(children[0])
        if node.type == "template_function":
            return " ".join(text(node).split())
        return declarator_name(node.child_by_field_name("declarator"))

    # Walk all conditional branches; choosing one platform's preprocessor
    # configuration here would silently omit obligations from other builds.
    stack = [(tree.root_node, ())]
    while stack:
        node, scope = stack.pop()
        if node.type == "ERROR" or node.is_missing:
            diagnostics.append(
                {
                    "kind": "missing_syntax" if node.is_missing else "unparsed_syntax",
                    "line": node.start_point.row + 1,
                    "column": node.start_point.column + 1,
                    "end_line": node.end_point.row + 1,
                    "node_type": node.type,
                }
            )
        if node.type == "function_definition":
            name = declarator_name(node.child_by_field_name("declarator"))
            if name is None:
                diagnostics.append(
                    {
                        "kind": "unnamed_definition",
                        "line": node.start_point.row + 1,
                        "column": node.start_point.column + 1,
                        "end_line": node.end_point.row + 1,
                        "node_type": node.type,
                    }
                )
            else:
                qualified = (
                    name.removeprefix("::")
                    if name.startswith("::")
                    else "::".join((*scope, name))
                )
                symbols.append({"name": qualified, "line": node.start_point.row + 1})
        if node.type in {
            "namespace_definition",
            "class_specifier",
            "struct_specifier",
            "union_specifier",
        }:
            name = node.child_by_field_name("name")
            scope = (*scope, text(name) if name else "(anonymous)")
        stack.extend((child, scope) for child in reversed(node.children))
    return CppDefinitionInventory(symbols, diagnostics)
