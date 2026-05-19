; tier-03 syntactic-fact query — Java.
; node-type reference: https://github.com/tree-sitter/tree-sitter-java/blob/master/src/node-types.json

(class_declaration
  name: (identifier) @name) @def.class

(interface_declaration
  name: (identifier) @name) @def.interface

(enum_declaration
  name: (identifier) @name) @def.enum

(record_declaration
  name: (identifier) @name) @def.record

(method_declaration
  name: (identifier) @name) @def.method

(import_declaration
  (scoped_identifier) @import.path) @import

(method_invocation
  name: (identifier) @call.callee)
