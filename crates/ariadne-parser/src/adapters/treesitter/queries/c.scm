; tier-11 syntactic-fact query — C.
; node-type reference: https://github.com/tree-sitter/tree-sitter-c/blob/v0.24.2/src/node-types.json

(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @def.function

(struct_specifier
  name: (type_identifier) @name
  body: (_)) @def.struct

(enum_specifier
  name: (type_identifier) @name
  body: (_)) @def.enum

(type_definition
  declarator: (type_identifier) @name) @def.type

(call_expression
  function: (identifier) @call.callee)

(call_expression
  function: (field_expression
    field: (field_identifier) @call.callee))

; tier-04 visibility / attribute captures.
;   @visibility  `storage_class_specifier` — `static` collapses to
;                `Private` (translation-unit-local); other classes leave
;                the lattice at `Unknown`.
;   @attribute   the C23 / GCC `attribute_declaration` form.

(storage_class_specifier) @visibility

(attribute_declaration) @attribute
