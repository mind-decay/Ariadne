//! End-to-end harness: the repo manifest, the performance budgets, and the
//! helpers the per-language suites call to drive the real `ariadne` binary
//! (src: .claude/plans/ariadne-core/tier-10-cli-e2e.md `<files>`).
//!
//! Fixture repositories are shallow-cloned at a pinned SHA via the system
//! `git` binary — the tier letter named the `git2` crate, but a SHA-pinned
//! shallow fetch against GitHub is markedly more reliable through the stock
//! client, and a test harness gains nothing from linking `libgit2`.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use ariadne_core::Lang;
use serde::Deserialize;
use serde_json::{Value, json};

/// One pinned fixture repository.
#[derive(Debug, Clone, Deserialize)]
pub struct RepoSpec {
    /// HTTPS clone URL.
    pub url: String,
    /// Frozen commit SHA fetched shallowly.
    pub sha: String,
}

/// The whole `fixtures/repos.toml` manifest, keyed by language tag.
pub type RepoManifest = BTreeMap<String, RepoSpec>;

/// Performance budgets enforced by the SLO suite — the v1 release gate
/// (src: .claude/plans/ariadne-core/plan.md `<constraints>`).
#[derive(Debug, Clone, Copy)]
pub struct PerfBudget {
    /// Maximum cold full-index wall-clock time.
    pub cold_index: Duration,
    /// Maximum p95 incremental-update apply latency.
    pub incremental_p95: Duration,
    /// Maximum p95 query latency.
    pub query_p95: Duration,
}

impl PerfBudget {
    /// The v1 SLOs: cold < 60 s, incremental p95 < 500 ms, query p95 < 100 ms.
    pub const V1: Self = Self {
        cold_index: Duration::from_secs(60),
        incremental_p95: Duration::from_millis(500),
        query_p95: Duration::from_millis(100),
    };
}

/// Parsed JSON-line summary printed by `ariadne index`.
#[derive(Debug, Clone, Deserialize)]
pub struct IndexReport {
    /// Files committed.
    pub files: usize,
    /// Symbols materialised.
    pub symbols: usize,
    /// Edges resolved.
    pub edges: usize,
    /// Language tags encountered.
    pub langs: Vec<String>,
    /// SCIP indexer successes, by lang tag.
    pub scip_successes: Vec<String>,
    /// SCIP indexers missing on PATH, by binary name.
    pub scip_missing: Vec<String>,
    /// Files whose parse aborted.
    pub parse_failures: usize,
    /// Persisted revision.
    pub revision: u64,
    /// Cold-index wall-clock duration, milliseconds.
    pub elapsed_ms: u128,
    /// Peak resident set size of the `ariadne index` process, in bytes. Not
    /// part of the JSON summary — populated by [`run_index_measured`] from a
    /// `/usr/bin/time` probe; `0` after a plain [`run_index`]
    /// [src: .claude/plans/ariadne-core/tier-12-parallel-cold-index.md step 6].
    #[serde(default)]
    pub peak_rss_bytes: u64,
}

impl IndexReport {
    /// True when the index holds at least one file, symbol, and edge.
    #[must_use]
    pub fn is_non_empty(&self) -> bool {
        self.files > 0 && self.symbols > 0 && self.edges > 0
    }

    /// True when `lang_tag` is a valid [`Lang`] tag that the index saw.
    #[must_use]
    pub fn indexed(&self, lang_tag: &str) -> bool {
        Lang::from_tag(lang_tag).is_some() && self.langs.iter().any(|t| t == lang_tag)
    }

    /// Cold-index wall-clock as a [`Duration`].
    #[must_use]
    pub fn cold_index(&self) -> Duration {
        Duration::from_millis(u64::try_from(self.elapsed_ms).unwrap_or(u64::MAX))
    }
}

/// Absolute path to the `ariadne` binary, building it on demand so the suite
/// runs standalone (`cargo nextest run -p ariadne-e2e`).
///
/// # Panics
/// Panics if `cargo build -p ariadne-cli` fails or the binary is still
/// absent afterwards — a hard environment fault the suite cannot proceed past.
#[must_use]
pub fn ariadne_binary() -> PathBuf {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let binary = target_dir().join(profile).join("ariadne");
    if !binary.exists() {
        let mut build = Command::new(env!("CARGO"));
        build.args(["build", "-p", "ariadne-cli"]);
        if profile == "release" {
            build.arg("--release");
        }
        let status = build.status().expect("spawn `cargo build -p ariadne-cli`");
        assert!(status.success(), "cargo build -p ariadne-cli failed");
    }
    assert!(
        binary.exists(),
        "ariadne binary missing after build: {}",
        binary.display()
    );
    binary
}

/// Workspace `target/` directory, honouring `CARGO_TARGET_DIR`.
fn target_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("CARGO_TARGET_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root is two levels above the e2e crate")
        .join("target")
}

/// Load and parse `fixtures/repos.toml`.
///
/// # Errors
/// Fails when the manifest is unreadable or malformed.
pub fn load_manifest() -> Result<RepoManifest> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("repos.toml");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

/// Look up one fixture repository by language tag.
///
/// # Errors
/// Fails when the manifest cannot be loaded or has no entry for `lang`.
pub fn repo_spec(lang: &str) -> Result<RepoSpec> {
    load_manifest()?
        .remove(lang)
        .ok_or_else(|| anyhow!("fixtures/repos.toml has no `{lang}` entry"))
}

/// Shallow-fetch `spec.sha` into `dest` via the system `git` binary.
///
/// # Errors
/// Propagates any non-zero `git` exit.
pub fn shallow_clone(spec: &RepoSpec, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest).context("create clone destination")?;
    git(dest, &["init", "--quiet"])?;
    git(dest, &["remote", "add", "origin", &spec.url])?;
    git(
        dest,
        &["fetch", "--depth", "1", "--quiet", "origin", &spec.sha],
    )?;
    git(dest, &["checkout", "--quiet", "FETCH_HEAD"])?;
    Ok(())
}

/// Run `git` in `cwd`, failing on a non-zero exit.
fn git(cwd: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("spawn git {args:?}"))?;
    if !output.status.success() {
        bail!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// Run `ariadne init` against `root`.
///
/// # Errors
/// Fails when the binary exits non-zero.
pub fn run_init(root: &Path) -> Result<()> {
    let output = Command::new(ariadne_binary())
        .arg("init")
        .arg(root)
        .output()
        .context("spawn `ariadne init`")?;
    if !output.status.success() {
        bail!(
            "ariadne init failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// Run `ariadne setup` against `root`.
///
/// # Errors
/// Fails when the binary exits non-zero.
pub fn run_setup(root: &Path) -> Result<()> {
    let output = Command::new(ariadne_binary())
        .arg("setup")
        .arg(root)
        .output()
        .context("spawn `ariadne setup`")?;
    if !output.status.success() {
        bail!(
            "ariadne setup failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// Run `ariadne index` against `root` and parse the JSON-line summary.
///
/// # Errors
/// Fails when the binary exits non-zero or prints no JSON summary line.
pub fn run_index(root: &Path) -> Result<IndexReport> {
    let output = Command::new(ariadne_binary())
        .arg("index")
        .arg(root)
        .output()
        .context("spawn `ariadne index`")?;
    if !output.status.success() {
        bail!(
            "ariadne index failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .ok_or_else(|| anyhow!("`ariadne index` printed no JSON summary"))?;
    serde_json::from_str(line).context("parse index summary JSON")
}

/// Run `ariadne index` under `/usr/bin/time`, parsing both the JSON summary
/// from stdout and the process peak RSS from the `time` report on stderr.
/// macOS `time -l` reports maxrss in bytes; GNU `time -v` reports it in
/// kbytes — the result is normalised to bytes either way
/// [src: .claude/plans/ariadne-core/tier-12-parallel-cold-index.md step 6].
///
/// # Errors
/// Fails when the binary exits non-zero, prints no JSON summary, or the
/// `/usr/bin/time` output carries no recognisable peak-RSS line.
pub fn run_index_measured(root: &Path) -> Result<IndexReport> {
    let time_flag = if cfg!(target_os = "macos") {
        "-l"
    } else {
        "-v"
    };
    let output = Command::new("/usr/bin/time")
        .arg(time_flag)
        .arg(ariadne_binary())
        .arg("index")
        .arg(root)
        .output()
        .context("spawn `/usr/bin/time ariadne index`")?;
    if !output.status.success() {
        bail!(
            "ariadne index (timed) failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .ok_or_else(|| anyhow!("`ariadne index` printed no JSON summary"))?;
    let mut report: IndexReport = serde_json::from_str(line).context("parse index summary JSON")?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    report.peak_rss_bytes = parse_peak_rss(&stderr)
        .ok_or_else(|| anyhow!("`/usr/bin/time` reported no peak RSS:\n{stderr}"))?;
    Ok(report)
}

/// Parse the peak-RSS figure from `/usr/bin/time` stderr, normalised to bytes.
/// macOS `time -l`: `<n>  maximum resident set size` — `n` already in bytes.
/// GNU `time -v`: `Maximum resident set size (kbytes): <n>` — `n` in kbytes
/// [src: <https://www.baeldung.com/linux/process-peak-memory-usage>].
fn parse_peak_rss(stderr: &str) -> Option<u64> {
    let line = stderr
        .lines()
        .find(|l| l.to_ascii_lowercase().contains("maximum resident set size"))?;
    if cfg!(target_os = "macos") {
        line.split_whitespace().next()?.parse().ok()
    } else {
        let kbytes: u64 = line.rsplit(':').next()?.trim().parse().ok()?;
        Some(kbytes.saturating_mul(1024))
    }
}

/// Run `ariadne query <tool> <args>` against `root`, returning its stdout.
///
/// # Errors
/// Fails when the binary exits non-zero.
pub fn run_query(root: &Path, tool: &str, args_json: &str) -> Result<String> {
    let output = Command::new(ariadne_binary())
        .args(["query", tool, args_json, "--root"])
        .arg(root)
        .output()
        .context("spawn `ariadne query`")?;
    if !output.status.success() {
        bail!(
            "ariadne query {tool} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Shallow-clone the `lang` fixture into `dest`, then run `ariadne init` and
/// `ariadne index` against it, returning the parsed index summary.
///
/// # Errors
/// Propagates manifest-lookup, clone, init, and index failures.
pub fn clone_init_index(lang: &str, dest: &Path) -> Result<IndexReport> {
    let spec = repo_spec(lang)?;
    shallow_clone(&spec, dest)?;
    run_init(dest)?;
    run_index(dest)
}

/// Clone, init, and index the `lang` fixture into `dest`, asserting the graph
/// is non-empty, covers `lang`, and indexed within the cold-index SLO.
///
/// The per-language suites call this as their whole body — a panic carries
/// the diagnostic [src: .claude/plans/ariadne-core/tier-10-cli-e2e.md step 10].
///
/// # Panics
/// Panics when the pipeline fails or any assertion is unmet.
pub fn verify_fixture_index(lang: &str, dest: &Path) {
    let report =
        clone_init_index(lang, dest).unwrap_or_else(|e| panic!("index `{lang}` fixture: {e:#}"));
    assert!(
        report.is_non_empty(),
        "`{lang}` fixture produced an empty graph: {report:?}"
    );
    assert!(
        report.indexed(lang),
        "`{lang}` fixture indexed no {lang} files: langs={:?}",
        report.langs
    );
    let budget = PerfBudget::V1.cold_index;
    let elapsed = report.cold_index();
    assert!(
        elapsed < budget,
        "`{lang}` cold index took {elapsed:?}, over the {budget:?} SLO"
    );
}

/// Distinct component files `verify_framework_fixture` probes for a
/// `Renders` edge before giving up — a real component app surfaces one in
/// its first handful of components.
const FRAMEWORK_RENDER_PROBE: usize = 64;

/// Clone, init, and index the JS-framework fixture keyed `repo` into `dest`,
/// then assert via the MCP query surface that the index carries `Component`
/// symbols and at least one `Renders` edge — the tier-09 component-graph
/// contract for React / Vue / Svelte / Astro
/// [src: .claude/plans/js-framework-support/tier-09-component-graph-e2e.md step 5].
///
/// # Panics
/// Panics when the pipeline fails or any assertion is unmet.
pub fn verify_framework_fixture(repo: &str, dest: &Path) {
    let report =
        clone_init_index(repo, dest).unwrap_or_else(|e| panic!("index `{repo}` fixture: {e:#}"));
    assert!(
        report.is_non_empty(),
        "`{repo}` fixture produced an empty graph: {report:?}"
    );

    let mut client = McpClient::connect(dest)
        .unwrap_or_else(|e| panic!("connect MCP client for `{repo}`: {e:#}"));

    // `Component` symbols — `DeclKind::Component` decls and the synthesized
    // per-file SFC component both carry the `component` kind tag.
    let listed = client
        .call_tool(
            "list_symbols",
            &json!({ "kind": "component", "limit": 256 }),
        )
        .unwrap_or_else(|e| panic!("list_symbols on `{repo}`: {e:#}"));
    let rows: Vec<Value> = serde_json::from_str(&tool_text(&listed).expect("list_symbols text"))
        .expect("parse list_symbols rows");
    assert!(
        !rows.is_empty(),
        "`{repo}` fixture produced no `Component` symbols",
    );

    // `Renders` edges — surfaced per component by `file_summary`. Probe the
    // distinct component files until one reports a rendered child.
    let mut files: Vec<String> = rows
        .iter()
        .filter_map(|r| r.get("file").and_then(Value::as_str).map(str::to_owned))
        .collect();
    files.sort();
    files.dedup();
    let probed = files.len().min(FRAMEWORK_RENDER_PROBE);
    let renders_found = files.iter().take(FRAMEWORK_RENDER_PROBE).any(|file| {
        let summary = client
            .call_tool("file_summary", &json!({ "path": file }))
            .unwrap_or_else(|e| panic!("file_summary `{file}` on `{repo}`: {e:#}"));
        let v: Value = serde_json::from_str(&tool_text(&summary).expect("file_summary text"))
            .expect("parse file_summary output");
        v["components"].as_array().is_some_and(|components| {
            components
                .iter()
                .any(|c| c["renders"].as_array().is_some_and(|r| !r.is_empty()))
        })
    });
    assert!(
        renders_found,
        "`{repo}` fixture produced no `Renders` edges across {probed} component files",
    );
}

/// The `p`-th percentile of `samples` (`p` in `0.0..=100.0`). Sorts in place.
///
/// # Panics
/// Panics when `samples` is empty.
#[must_use]
pub fn percentile(samples: &mut [Duration], p: f64) -> Duration {
    assert!(!samples.is_empty(), "percentile of an empty sample set");
    samples.sort_unstable();
    // Nearest-rank: rank = ceil(p/100 * n), clamped into the slice. Sample
    // sets here run to at most a few hundred entries, so the f64 round-trip
    // of the count is exact.
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let rank = (p / 100.0 * samples.len() as f64).ceil() as usize;
    let idx = rank.saturating_sub(1).min(samples.len() - 1);
    samples[idx]
}

/// Collect up to `limit` recognised source-file paths under `root`, used to
/// drive incremental-edit probes. Walks `.git` / `node_modules` / `target`
/// aside so the probe never mutates VCS or build artefacts.
#[must_use]
pub fn collect_source_files(root: &Path, limit: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if out.len() >= limit {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                if !matches!(
                    entry.file_name().to_str(),
                    Some(".git" | "node_modules" | "target" | ".ariadne")
                ) {
                    stack.push(entry.path());
                }
            } else if file_type.is_file() && is_source_ext(&entry.path()) {
                out.push(entry.path());
                if out.len() >= limit {
                    break;
                }
            }
        }
    }
    out
}

/// True when `path` carries a recognised tree-sitter source extension. The
/// list tracks the ten grammars the indexer actually re-derives, so the
/// incremental probe mutates files the watcher genuinely tracks — C/C++
/// included since tier-11 [src: crates/ariadne-cli/src/domain/mod.rs
/// `lang_for_path`].
fn is_source_ext(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some(
            "rs" | "ts"
                | "tsx"
                | "js"
                | "jsx"
                | "py"
                | "go"
                | "java"
                | "kt"
                | "cs"
                | "c"
                | "h"
                | "cpp"
                | "cc"
                | "cxx"
                | "hpp"
                | "hh"
                | "hxx"
        )
    )
}

/// A pending MCP response outliving this bound is a transport fault, not a
/// slow query — the p95 assertion is the real latency gate. Generous enough
/// to cover a cold catalog build over a 100K-file index before the server
/// first reads stdin.
const MCP_RECV_TIMEOUT: Duration = Duration::from_secs(120);

/// A minimal synchronous MCP client over a spawned `ariadne serve` child.
///
/// Speaks newline-delimited JSON-RPC on the child's stdio — the MCP stdio
/// transport framing. A background thread drains stdout into a channel so a
/// slow or hung server can never deadlock the test on a full OS pipe.
#[derive(Debug)]
pub struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout_rx: Receiver<String>,
    next_id: i64,
}

impl McpClient {
    /// Spawn `ariadne serve <root>` and complete the MCP handshake
    /// (`initialize` request + `notifications/initialized`).
    ///
    /// # Errors
    /// Fails when the child cannot spawn or the `initialize` response is not
    /// a well-formed result frame.
    pub fn connect(root: &Path) -> Result<Self> {
        let mut child = Command::new(ariadne_binary())
            .arg("serve")
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("spawn `ariadne serve`")?;
        let stdin = child.stdin.take().context("capture serve stdin")?;
        let stdout = child.stdout.take().context("capture serve stdout")?;
        let (tx, stdout_rx) = channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut client = Self {
            child,
            stdin,
            stdout_rx,
            next_id: 1,
        };
        client.handshake()?;
        Ok(client)
    }

    /// Send `initialize` then the `notifications/initialized` notification.
    fn handshake(&mut self) -> Result<()> {
        let init = self.request(
            "initialize",
            &json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "ariadne-e2e", "version": "0.0.0" },
            }),
        )?;
        init.get("serverInfo")
            .context("`initialize` result missing `serverInfo`")?;
        self.notify("notifications/initialized", &json!({}))
    }

    /// List the tool names the server advertises on `tools/list`.
    ///
    /// # Errors
    /// Propagates transport and JSON-RPC error frames.
    pub fn list_tools(&mut self) -> Result<Vec<String>> {
        let result = self.request("tools/list", &json!({}))?;
        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .context("`tools/list` result missing `tools` array")?;
        Ok(tools
            .iter()
            .filter_map(|t| t.get("name").and_then(Value::as_str))
            .map(str::to_owned)
            .collect())
    }

    /// Invoke one tool, returning the `CallToolResult` frame.
    ///
    /// # Errors
    /// Fails on a JSON-RPC error frame or a tool-level `isError` result.
    pub fn call_tool(&mut self, name: &str, arguments: &Value) -> Result<Value> {
        let result = self.request(
            "tools/call",
            &json!({ "name": name, "arguments": arguments }),
        )?;
        if result.get("isError").and_then(Value::as_bool) == Some(true) {
            bail!("tool `{name}` returned an `isError` result: {result}");
        }
        Ok(result)
    }

    /// Send a JSON-RPC request and block for its matching response.
    ///
    /// # Errors
    /// Fails on a transport break, a recv timeout, or a JSON-RPC error frame.
    pub fn request(&mut self, method: &str, params: &Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let frame = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        writeln!(self.stdin, "{frame}").with_context(|| format!("send `{method}`"))?;
        self.stdin.flush().context("flush serve stdin")?;
        loop {
            let line = self
                .stdout_rx
                .recv_timeout(MCP_RECV_TIMEOUT)
                .map_err(|e| match e {
                    RecvTimeoutError::Timeout => {
                        anyhow!("`{method}` timed out after {MCP_RECV_TIMEOUT:?}")
                    }
                    RecvTimeoutError::Disconnected => {
                        anyhow!("`ariadne serve` closed stdout before answering `{method}`")
                    }
                })?;
            let msg: Value = serde_json::from_str(&line)
                .with_context(|| format!("parse JSON-RPC frame: {line}"))?;
            if msg.get("id").and_then(Value::as_i64) != Some(id) {
                continue; // server notification or stale frame — skip
            }
            if let Some(err) = msg.get("error") {
                bail!("`{method}` returned a JSON-RPC error frame: {err}");
            }
            return Ok(msg.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    /// Send a JSON-RPC notification (no id, no response expected).
    fn notify(&mut self, method: &str, params: &Value) -> Result<()> {
        let frame = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        writeln!(self.stdin, "{frame}").with_context(|| format!("send notification `{method}`"))?;
        self.stdin.flush().context("flush serve stdin")
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Extract the `content[0].text` payload from an MCP `CallToolResult` frame —
/// every Ariadne tool wires its output as a single JSON text block.
///
/// # Errors
/// Fails when the frame carries no textual content block.
pub fn tool_text(result: &Value) -> Result<String> {
    result
        .get("content")
        .and_then(Value::as_array)
        .and_then(|blocks| blocks.first())
        .and_then(|block| block.get("text"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .context("MCP tool result missing `content[0].text`")
}
