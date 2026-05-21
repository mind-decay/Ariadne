// Minimal SCIP protobuf writer + symbol-descriptor helpers.
//
// The bridge emits only the slice of the SCIP schema `ariadne-scip` ingests:
// Index { Metadata, Document }, Document { Occurrence, SymbolInformation }.
// Field numbers track `crates/ariadne-scip/proto/scip.proto` at the SHA in
// `proto/SCIP_COMMIT`. A hand-written writer is used instead of vendoring
// `scip-typescript`'s generated bindings + its `google-protobuf` runtime — see
// docs/adr/0013-scip-sfc-bridge.md for the vendoring decision.

/**
 * Protobuf wire-format writer for the fixed field set the SCIP subset needs.
 * Only two wire types occur here: `0` varint and `2` length-delimited.
 */
export class ProtoWriter {
  private readonly chunks: Buffer[] = [];

  private varint(value: number): void {
    const bytes: number[] = [];
    let v = value;
    do {
      let b = v & 0x7f;
      v = Math.floor(v / 0x80);
      if (v > 0) {
        b |= 0x80;
      }
      bytes.push(b);
    } while (v > 0);
    this.chunks.push(Buffer.from(bytes));
  }

  private tag(field: number, wire: number): void {
    this.varint(field * 8 + wire);
  }

  /** String field. proto3 omits the empty default. */
  string(field: number, value: string): void {
    if (value.length === 0) {
      return;
    }
    const b = Buffer.from(value, "utf8");
    this.tag(field, 2);
    this.varint(b.length);
    this.chunks.push(b);
  }

  /** Varint scalar field. proto3 omits the zero default. */
  int(field: number, value: number): void {
    if (value === 0) {
      return;
    }
    this.tag(field, 0);
    this.varint(value);
  }

  /** Length-delimited sub-message field. */
  message(field: number, body: Buffer): void {
    this.tag(field, 2);
    this.varint(body.length);
    this.chunks.push(body);
  }

  /** Packed `repeated int32` field — the SCIP `Occurrence.range` encoding. */
  packedInt32(field: number, values: readonly number[]): void {
    if (values.length === 0) {
      return;
    }
    const inner = new ProtoWriter();
    for (const v of values) {
      inner.varint(v);
    }
    this.message(field, inner.finish());
  }

  finish(): Buffer {
    return Buffer.concat(this.chunks);
  }
}

const SIMPLE_IDENTIFIER = /^[a-zA-Z0-9_$+-]+$/;

/**
 * Encode one descriptor name per the SCIP grammar: a name that fits
 * `<simple-identifier>` is emitted bare, otherwise it is backtick-escaped with
 * inner backticks doubled [src: crates/ariadne-scip/proto/scip.proto —
 * Descriptor].
 */
export function descriptorName(name: string): string {
  if (name.length > 0 && SIMPLE_IDENTIFIER.test(name)) {
    return name;
  }
  return "`" + name.replace(/`/g, "``") + "`";
}
