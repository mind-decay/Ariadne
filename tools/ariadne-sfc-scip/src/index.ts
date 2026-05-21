#!/usr/bin/env node
// ariadne-sfc-scip — Volar-based SCIP bridge for Vue single-file components.
//
// Wraps `ts.createProgram` with `@volar/typescript`'s `proxyCreateProgram` and
// the `@vue/language-core` language plugin so `.vue` files become
// program-visible TypeScript. Walks every `.vue` source file, resolves each
// identifier through the type checker, remaps the virtual-TS position back to
// the original SFC text via Volar's source maps, and emits a SCIP index that
// `ariadne-scip` ingests. See docs/adr/0013-scip-sfc-bridge.md.
//
// Usage: ariadne-sfc-scip --framework vue --cwd <root> --output <out.scip>

import * as fs from "fs";
import * as path from "path";
import * as ts from "typescript";
import * as vue from "@vue/language-core";
import type { Language, Mapper, SourceScript } from "@vue/language-core";
import { proxyCreateProgram } from "@volar/typescript";
import { ProtoWriter, descriptorName } from "./scip";

const SCHEME = "scip-vue-bridge";
const TOOL_VERSION = "0.1.0";
const ENTRY_NAME = "__ariadne_sfc_entry__.ts";
const DEFINITION_ROLE = 0x1;
const TYPE_FLAGS =
  ts.SymbolFlags.Class |
  ts.SymbolFlags.Interface |
  ts.SymbolFlags.Enum |
  ts.SymbolFlags.TypeAlias;

interface Args {
  framework: string;
  cwd: string;
  output: string;
}

interface Pkg {
  name: string;
  version: string;
}

interface Occurrence {
  range: number[];
  symbol: string;
  roles: number;
}

interface DocResult {
  relativePath: string;
  occurrences: Occurrence[];
  definedSymbols: string[];
}

function fail(message: string): never {
  process.stderr.write(`ariadne-sfc-scip: ${message}\n`);
  process.exit(1);
}

function parseArgs(argv: string[]): Args {
  const out: Partial<Args> = {};
  for (let i = 0; i < argv.length; i++) {
    const flag = argv[i];
    if (flag === "--framework") {
      out.framework = argv[++i];
    } else if (flag === "--cwd") {
      out.cwd = argv[++i];
    } else if (flag === "--output") {
      out.output = argv[++i];
    } else {
      fail(`unknown argument: ${flag}`);
    }
  }
  if (!out.framework) fail("missing --framework");
  if (!out.cwd) fail("missing --cwd");
  if (!out.output) fail("missing --output");
  return out as Args;
}

function toPosix(p: string): string {
  return p.split(path.sep).join("/");
}

function findConfig(cwd: string): string {
  for (const name of ["tsconfig.json", "jsconfig.json"]) {
    const candidate = path.join(cwd, name);
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }
  return fail(`no tsconfig.json or jsconfig.json in ${cwd}`);
}

function readPackage(cwd: string): Pkg {
  try {
    const raw = JSON.parse(fs.readFileSync(path.join(cwd, "package.json"), "utf8"));
    const name = String(raw.name ?? "project").replace(/\s+/g, "-");
    const version = String(raw.version ?? "0.0.0").replace(/\s+/g, "-");
    return { name, version };
  } catch {
    return { name: "project", version: "0.0.0" };
  }
}

function pathToFileUri(p: string): string {
  let normalized = toPosix(p);
  if (!normalized.startsWith("/")) {
    normalized = "/" + normalized;
  }
  return "file://" + encodeURI(normalized);
}

function computeLineStarts(text: string): number[] {
  const starts = [0];
  for (let i = 0; i < text.length; i++) {
    if (text.charCodeAt(i) === 10) {
      starts.push(i + 1);
    }
  }
  return starts;
}

function offsetToPosition(lineStarts: number[], offset: number): [number, number] {
  let lo = 0;
  let hi = lineStarts.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (lineStarts[mid] <= offset) {
      lo = mid;
    } else {
      hi = mid - 1;
    }
  }
  return [lo, offset - lineStarts[lo]];
}

/** First declaration of `symbol` that lives in a non-dependency project file. */
function projectDeclaration(symbol: ts.Symbol, cwd: string): ts.Declaration | undefined {
  for (const decl of symbol.declarations ?? []) {
    const file = decl.getSourceFile().fileName;
    if (file.startsWith(cwd) && !file.includes("/node_modules/")) {
      return decl;
    }
  }
  return undefined;
}

function isExported(symbol: ts.Symbol, decl: ts.Declaration): boolean {
  if (symbol.getName() === "default") {
    return true;
  }
  return (ts.getCombinedModifierFlags(decl) & ts.ModifierFlags.Export) !== 0;
}

/**
 * Stable SCIP symbol string for `symbol`. Module-exported symbols get a global
 * symbol keyed on their declaration file + name so a definition in one `.vue`
 * and a reference in another resolve to the same string; everything else gets
 * a document-local `local <n>` symbol.
 */
function symbolString(
  symbol: ts.Symbol,
  cwd: string,
  pkg: Pkg,
  localIds: Map<ts.Symbol, string>,
): string {
  const decl = projectDeclaration(symbol, cwd);
  if (decl && isExported(symbol, decl)) {
    const rel = toPosix(path.relative(cwd, decl.getSourceFile().fileName));
    if (!rel.startsWith("..")) {
      const segments = rel
        .split("/")
        .map((seg) => descriptorName(seg) + "/")
        .join("");
      const suffix = (symbol.flags & TYPE_FLAGS) !== 0 ? "#" : ".";
      return `${SCHEME} npm ${pkg.name} ${pkg.version} ${segments}${descriptorName(symbol.getName())}${suffix}`;
    }
  }
  let local = localIds.get(symbol);
  if (local === undefined) {
    local = `local ${localIds.size}`;
    localIds.set(symbol, local);
  }
  return local;
}

function compareOccurrence(a: Occurrence, b: Occurrence): number {
  for (let i = 0; i < 4; i++) {
    if (a.range[i] !== b.range[i]) {
      return a.range[i] - b.range[i];
    }
  }
  if (a.symbol !== b.symbol) {
    return a.symbol < b.symbol ? -1 : 1;
  }
  return a.roles - b.roles;
}

/**
 * Walk one `.vue` virtual source file, remap occurrences onto SFC text.
 *
 * `localIds` is shared across every document so `local <n>` symbols are unique
 * across the whole index — SCIP `local` symbols are document-scoped, and a
 * shared counter keeps two unrelated locals from colliding on one id.
 */
function indexDocument(
  sf: ts.SourceFile,
  checker: ts.TypeChecker,
  language: Language<string>,
  cwd: string,
  pkg: Pkg,
  localIds: Map<ts.Symbol, string>,
): DocResult | undefined {
  const sourceScript: SourceScript<string> | undefined = language.scripts.get(sf.fileName);
  const tsPlugin = sourceScript?.generated?.languagePlugin.typescript;
  if (!sourceScript?.generated || !tsPlugin) {
    return undefined;
  }
  const serviceScript = tsPlugin.getServiceScript(sourceScript.generated.root);
  if (!serviceScript) {
    return undefined;
  }
  const map: Mapper = language.maps.get(serviceScript.code, sourceScript);
  const leadingOffset = serviceScript.preventLeadingOffset
    ? 0
    : sourceScript.snapshot.getLength();
  const srcText = sourceScript.snapshot.getText(0, sourceScript.snapshot.getLength());
  const lineStarts = computeLineStarts(srcText);

  const byKey = new Map<string, Occurrence>();
  const defined = new Set<string>();

  const visit = (node: ts.Node): void => {
    if (ts.isIdentifier(node)) {
      let symbol = checker.getSymbolAtLocation(node);
      if (symbol) {
        if (symbol.flags & ts.SymbolFlags.Alias) {
          try {
            symbol = checker.getAliasedSymbol(symbol);
          } catch {
            // keep the unresolved alias symbol
          }
        }
        const vStart = node.getStart(sf) - leadingOffset;
        const vEnd = node.getEnd() - leadingOffset;
        if (vStart >= 0 && vEnd >= vStart) {
          let mapped: [number, number] | undefined;
          for (const [s, e] of map.toSourceRange(
            vStart,
            vEnd,
            false,
            (info) => info.navigation !== false,
          )) {
            mapped = [s, e];
            break;
          }
          // Keep an occurrence only when the remapped source span exactly
          // covers the identifier — same width as the generated token and on
          // one line. This drops template-codegen identifiers whose mapping
          // fans out to a whole `<script>` block or `<Component/>` element.
          if (mapped && mapped[1] - mapped[0] === vEnd - vStart && mapped[1] > mapped[0]) {
            const [startLine, startChar] = offsetToPosition(lineStarts, mapped[0]);
            const [endLine, endChar] = offsetToPosition(lineStarts, mapped[1]);
            if (startLine === endLine) {
              const sym = symbolString(symbol, cwd, pkg, localIds);
              const isDef = (symbol.declarations ?? []).some(
                (d) => ts.getNameOfDeclaration(d) === node,
              );
              const roles = isDef ? DEFINITION_ROLE : 0;
              const range = [startLine, startChar, endLine, endChar];
              const key = `${range.join(",")}|${sym}|${roles}`;
              if (!byKey.has(key)) {
                byKey.set(key, { range, symbol: sym, roles });
                if (isDef) {
                  defined.add(sym);
                }
              }
            }
          }
        }
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sf);

  const occurrences = [...byKey.values()].sort(compareOccurrence);
  if (occurrences.length === 0) {
    return undefined;
  }
  return {
    relativePath: toPosix(path.relative(cwd, sf.fileName)),
    occurrences,
    definedSymbols: [...defined].sort(),
  };
}

function encodeIndex(cwd: string, pkg: Pkg, documents: DocResult[]): Buffer {
  const index = new ProtoWriter();

  const metadata = new ProtoWriter();
  metadata.int(1, 0); // version = UnspecifiedProtocolVersion
  const toolInfo = new ProtoWriter();
  toolInfo.string(1, "ariadne-sfc-scip");
  toolInfo.string(2, TOOL_VERSION);
  toolInfo.string(3, "vue"); // arguments[0] — framework
  metadata.message(2, toolInfo.finish());
  metadata.string(3, pathToFileUri(cwd)); // project_root
  metadata.int(4, 1); // text_document_encoding = UTF8
  index.message(1, metadata.finish());

  for (const doc of documents) {
    const document = new ProtoWriter();
    document.string(1, doc.relativePath); // relative_path
    for (const occ of doc.occurrences) {
      const occurrence = new ProtoWriter();
      occurrence.packedInt32(1, occ.range);
      occurrence.string(2, occ.symbol);
      occurrence.int(3, occ.roles);
      document.message(2, occurrence.finish());
    }
    for (const symbol of doc.definedSymbols) {
      const info = new ProtoWriter();
      info.string(1, symbol);
      document.message(3, info.finish());
    }
    document.string(4, "Vue"); // language
    index.message(2, document.finish());
  }

  return index.finish();
}

function main(): void {
  const args = parseArgs(process.argv.slice(2));
  if (args.framework !== "vue") {
    fail(`unsupported framework '${args.framework}' (this build indexes vue)`);
  }
  const cwd = path.resolve(args.cwd);
  const configPath = findConfig(cwd);
  const pkg = readPackage(cwd);

  const extraExtensions: ts.FileExtensionInfo[] = [
    { extension: "vue", isMixedContent: true, scriptKind: ts.ScriptKind.Deferred },
  ];
  const configJson = ts.readConfigFile(configPath, ts.sys.readFile).config;
  const parsed = ts.parseJsonConfigFileContent(
    configJson,
    ts.sys,
    cwd,
    undefined,
    configPath,
    undefined,
    extraExtensions,
  );
  const vueFiles = parsed.fileNames.filter((f) => f.endsWith(".vue"));
  if (vueFiles.length === 0) {
    fail(`no .vue files resolved under ${cwd}`);
  }

  // TS createProgram rejects .vue root files; a synthetic entry that
  // side-effect-imports every .vue pulls them in via the proxy's resolver.
  const entryPath = path.join(cwd, ENTRY_NAME);
  const entryContent =
    vueFiles
      .map((f) => `import ${JSON.stringify("./" + toPosix(path.relative(cwd, f)))};`)
      .join("\n") + "\n";

  let language: Language<string> | undefined;
  const vueOptions = vue.getDefaultCompilerOptions();
  const createProgram = proxyCreateProgram(ts, ts.createProgram, (tsModule, options) => {
    const plugin = vue.createVueLanguagePlugin<string>(
      tsModule,
      options.options,
      vueOptions,
      (id) => id,
    );
    return {
      languagePlugins: [plugin],
      setup(lang) {
        language = lang;
      },
    };
  });

  const host = ts.createCompilerHost(parsed.options);
  const baseGetSourceFile = host.getSourceFile.bind(host);
  const baseReadFile = host.readFile.bind(host);
  const baseFileExists = host.fileExists.bind(host);
  host.getSourceFile = (fileName, languageVersion, onError, shouldCreate) => {
    if (fileName === entryPath) {
      return ts.createSourceFile(fileName, entryContent, languageVersion, true, ts.ScriptKind.TS);
    }
    return baseGetSourceFile(fileName, languageVersion, onError, shouldCreate);
  };
  host.readFile = (fileName) =>
    fileName === entryPath ? entryContent : baseReadFile(fileName);
  host.fileExists = (fileName) =>
    fileName === entryPath ? true : baseFileExists(fileName);

  const program = createProgram({
    rootNames: [entryPath],
    options: parsed.options,
    host,
  });
  if (!language) {
    fail("Volar language layer was not initialized");
  }
  const checker = program.getTypeChecker();

  const localIds = new Map<ts.Symbol, string>();
  const documents = program
    .getSourceFiles()
    .filter((sf) => sf.fileName.endsWith(".vue"))
    .sort((a, b) => (a.fileName < b.fileName ? -1 : a.fileName > b.fileName ? 1 : 0))
    .map((sf) => indexDocument(sf, checker, language as Language<string>, cwd, pkg, localIds))
    .filter((doc): doc is DocResult => doc !== undefined);

  if (documents.length === 0) {
    fail("no .vue documents produced occurrences");
  }
  fs.writeFileSync(args.output, encodeIndex(cwd, pkg, documents));
}

main();
