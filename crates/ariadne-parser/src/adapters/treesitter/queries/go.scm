; tier-03 syntactic-fact query — Go.
; node-type reference: https://github.com/tree-sitter/tree-sitter-go/blob/master/src/node-types.json

(function_declaration
  name: (identifier) @name) @def.function

(method_declaration
  name: (field_identifier) @name) @def.method

(type_declaration
  (type_spec
    name: (type_identifier) @name)) @def.type

(import_spec
  path: (interpreted_string_literal) @import.path) @import

(call_expression
  function: (identifier) @call.free)

(call_expression
  function: (selector_expression
    field: (field_identifier) @call.method))
