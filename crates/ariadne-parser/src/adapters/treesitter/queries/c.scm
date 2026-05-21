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
