# Formula semantics

Formulas are stored with their leading `=` and evaluated by `crates/formula`. Cell references are 1-based (`A1` is row 1, column 1). Empty cells evaluate as empty values. Division by zero returns `#DIV/0!`; malformed names and unsupported operations return typed formula errors rather than panicking.

Supported function behavior is defined by the registry in `crates/formula/src/functions.rs`. Range arguments are flattened in row-major order. Formula evaluation is deterministic and must remain independent of the frontend.
