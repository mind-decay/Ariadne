; tier-11 syntactic-fact query — C++.
; node-type reference: https://github.com/tree-sitter/tree-sitter-cpp/blob/v0.23.4/src/node-types.json

(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @def.function

(function_definition
  declarator: (function_declarator
    declarator: (field_identifier) @name)) @def.method

(function_definition
  declarator: (function_declarator
    declarator: (qualified_identifier
      name: (identifier) @name))) @def.method

(struct_specifier
  name: (type_identifier) @name
  body: (_)) @def.struct

(class_specifier
  name: (type_identifier) @name
  body: (_)) @def.class

(enum_specifier
  name: (type_identifier) @name
  body: (_)) @def.enum

(type_definition
  declarator: (type_identifier) @name) @def.type

(namespace_definition
  name: (namespace_identifier) @name) @def.module

(call_expression
  function: (identifier) @call.callee)

(call_expression
  function: (field_expression
    field: (field_identifier) @call.callee))

(call_expression
  function: (qualified_identifier
    name: (identifier) @call.callee))
