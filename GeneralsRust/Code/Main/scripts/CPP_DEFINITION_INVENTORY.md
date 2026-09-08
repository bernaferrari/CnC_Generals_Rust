# C++ definition discovery

The provenance and review-packet generators use pinned Tree-sitter C++ grammar
packages to discover function definitions, including free functions, members,
operators, overloads and all preprocessor branches. Calls, comments and function
declarations do not count as definitions.

From the repository root, create a tooling environment once:

```sh
python3 -m venv .venv-parity
.venv-parity/bin/python -m pip install -r GeneralsRust/Code/Main/scripts/requirements-provenance.txt
```

Run the generators and their checks with that environment's Python. To use the
`python3` commands printed inside review packets, first run
`source .venv-parity/bin/activate`:

```sh
.venv-parity/bin/python GeneralsRust/Code/Main/scripts/generate_port_provenance.py
.venv-parity/bin/python GeneralsRust/Code/Main/scripts/generate_port_review_queue.py
.venv-parity/bin/python -m unittest discover -s GeneralsRust/Code/Main/scripts -p 'test_*.py'
```

The parser identity participates in the inventory digest. Missing or mismatched
packages fail explicitly; there is no fallback to the old regular expression.
CI installs the same requirements before running these tools.

`source.symbol_extraction_diagnostics` records unparsed or missing syntax by
line and column. A reviewed source with these diagnostics cannot receive valid
symbol-ownership credit, even if every recovered function has an assignment.
These diagnostics may reflect original macros or conditional syntax rather than
invalid C++. Resolve them by inspecting the original source and improving parser
support; suppressing them would hide unknown coverage.

A clean parse is still discovery evidence. Macro expansion can generate behavior
that is absent from the syntax tree. Whole-source review, explicit ownership,
state/data/layout comparison and executable C++/Rust behavior checks remain
separate requirements. Header review and full game playability are not established
by a list of function names.
