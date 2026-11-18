# ert-gwt, vendored

`ert-gwt.el` is a verbatim copy of
[ert-gwt](https://github.com/OverbearingPearl/ert-gwt) at commit
`108e396b9f85b2981b3400db18f0cf066fbe3ba1` (2026-11-17), GPL-3.0-or-later, kept
under its own license and header.

It is here because every test in this repository is a Given-When-Then story
(ADR 11) and the package is not on MELPA yet. It is the only vendored file in
the tree.

When `ert-gwt` lands on MELPA, delete this directory, remove the load-path line
from `test/pearl-support-tests.el`, and declare the dependency the way the other
development tools are declared. Nothing else refers to it.
