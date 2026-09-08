from pathlib import Path
import tempfile
import unittest

from generate_port_provenance import extract_cpp_symbols


class CppDefinitionInventoryTests(unittest.TestCase):
    def extract(self, source: str):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "sample.cpp"
            path.write_text(source)
            return extract_cpp_symbols(path)

    def test_definitions_include_free_functions_but_not_calls_or_declarations(self):
        source = """void declared();
void first() {
    Other::called();
    if (Other::condition()) { Other::nested(); }
}
int second(int value) { return Other::result(value); }
"""
        self.assertEqual(
            self.extract(source),
            [
                {"name": "first", "line": 2},
                {"name": "second", "line": 6},
            ],
        )

    def test_namespaces_members_operators_and_overloads_remain_distinct(self):
        source = """namespace Game {
struct Foo {
    Foo() : value{1} {}
    ~Foo() {}
    operator bool() const { return true; }
    int operator[](int index) { return index; }
    int value;
};
void Foo::update(int x) {}
void Foo::update(float x) {}
}
"""
        self.assertEqual(
            self.extract(source),
            [
                {"name": "Game::Foo::Foo", "line": 3},
                {"name": "Game::Foo::~Foo", "line": 4},
                {"name": "Game::Foo::operator bool", "line": 5},
                {"name": "Game::Foo::operator[]", "line": 6},
                {"name": "Game::Foo::update", "line": 9},
                {"name": "Game::Foo::update", "line": 10},
            ],
        )

    def test_all_conditional_branches_and_multiline_return_declarators(self):
        source = """#if ENABLE_FIRST
static const char *
first() { return "not_a_function() {}"; }
#else
void second() {}
#endif
// void commented() {}
"""
        self.assertEqual(
            self.extract(source),
            [
                {"name": "first", "line": 2},
                {"name": "second", "line": 5},
            ],
        )

    def test_reference_pointer_and_conversion_declarators_keep_function_identity(self):
        source = "const Foo & Foo::get() const {}\nFoo *& Foo::head() {}\nFoo::operator bool() const {}\n"
        self.assertEqual(
            self.extract(source),
            [
                {"name": "Foo::get", "line": 1},
                {"name": "Foo::head", "line": 2},
                {"name": "Foo::operator bool", "line": 3},
            ],
        )

    def test_syntax_errors_remain_visible_beside_recovered_definitions(self):
        from cpp_definition_inventory import extract_cpp_definition_inventory

        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "partial.cpp"
            path.write_text("void recovered() { @; }\n")
            result = extract_cpp_definition_inventory(path)
        self.assertEqual(result.symbols, [{"name": "recovered", "line": 1}])
        self.assertTrue(result.diagnostics)
        self.assertTrue(all(item["line"] == 1 for item in result.diagnostics))

    def test_original_winmain_entry_and_window_procedure_are_present(self):
        root = Path(__file__).resolve().parents[4]
        symbols = extract_cpp_symbols(root / "GeneralsMD/Code/Main/WinMain.cpp")
        self.assertEqual(
            [s["name"] for s in symbols],
            [
                "messageToString",
                "WndProc",
                "initializeAppWindows",
                "munkeeFunc",
                "checkProtection",
                "strtrim",
                "nextParam",
                "WinMain",
                "CreateGameEngine",
            ],
        )


if __name__ == "__main__":
    unittest.main()
