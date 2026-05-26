; tier-03 syntactic-fact query — Kotlin (tree-sitter-kotlin-ng 1.1).
; node-type reference: tree-sitter-kotlin-ng-1.1.0/src/node-types.json
; Notes:
;   * Kotlin grammar uses `identifier` (no `simple_identifier`).
;   * `import` (not `import_header`); module path is a child `qualified_identifier` or `identifier`.
;   * `call_expression` has no named callee field; match the leading
;     `expression` child whose inner node is an `identifier`.

(function_declaration
  name: (identifier) @name) @def.function

(class_declaration
  name: (identifier) @name) @def.class

(object_declaration
  name: (identifier) @name) @def.object

(import (qualified_identifier) @import.path) @import
(import (identifier) @import.path) @import

(call_expression (expression (identifier) @call.callee))

; tier-04 visibility / attribute captures.
;   @visibility  the `modifiers` node — text-scan picks the strongest
;                visibility keyword (`public`/`protected`/`internal`/
;                `private`).
;   @attribute   Kotlin `annotation` nodes preceding a decl.

(modifiers) @visibility

(annotation) @attribute
