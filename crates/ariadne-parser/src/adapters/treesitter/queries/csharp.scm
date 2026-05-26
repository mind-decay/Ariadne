; tier-03 syntactic-fact query — C#.
; node-type reference: https://github.com/tree-sitter/tree-sitter-c-sharp/blob/master/src/node-types.json

(class_declaration
  name: (identifier) @name) @def.class

(interface_declaration
  name: (identifier) @name) @def.interface

(struct_declaration
  name: (identifier) @name) @def.struct

(enum_declaration
  name: (identifier) @name) @def.enum

(record_declaration
  name: (identifier) @name) @def.record

(method_declaration
  name: (identifier) @name) @def.method

(using_directive
  (qualified_name) @import.path) @import

(using_directive
  (identifier) @import.path) @import

(invocation_expression
  function: (identifier) @call.callee)

(invocation_expression
  function: (member_access_expression
    name: (identifier) @call.callee))

; tier-04 visibility / attribute captures.
;   @visibility  the `modifier` token inside a decl
;                (`public`/`protected`/`internal`/`private`).
;   @attribute   `attribute_list` preceding a decl
;                (`[Test]`, `[Authorize(…)]`).

(modifier) @visibility

(attribute_list) @attribute
