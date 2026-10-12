# recodex

A **locally-built Codex CLI** produced from this fork and published as the `recodex`
command. It coexists with the normal (npm-installed) `codex` — `codex` is untouched.

- Branch: `recodex` (build branch). `main` holds the feature commits.
- Binary: `C:\Users\hp\.recodex\codex.exe`
- Shim on PATH: `C:\Users\hp\.cargo\bin\recodex.cmd`
- Fork: https://github.com/AmerDje/codex

---

## 1. How `recodex` is built and installed

### Build (GitHub Actions)
- Workflow: [`.github/workflows/build-recodex.yml`](.github/workflows/build-recodex.yml)
- Triggers: push to the `recodex` branch, or manual `workflow_dispatch`.
- Runner: `windows-latest`, Rust `1.95.0`.
- Lean release profile (to fit the runner): `lto=false`, `debug=0`, `strip=symbols`,
  `codegen-units=16`, `opt-level=1`, `CARGO_BUILD_JOBS=2`, `CARGO_INCREMENTAL=0`.
- Builds `--bin codex` and publishes a public GitHub Release `recodex-<run_number>`
  with `codex.exe` (also uploaded as a workflow artifact).
- `GITHUB_TOKEN` has `contents: write` so it can publish the release asset
  (that lets it be downloaded without auth).

> The `recodex` branch intentionally differs from `main` in the workflow only. Other
> upstream workflows (`blocking-ci`, `postmerge-ci`) are disabled on the fork, and the
> build workflow is scoped to the `recodex` branch so it does not collide with upstream CI.

### Download & install
1. Wait for the `build-recodex` run to finish on
   https://github.com/AmerDje/codex/actions
2. Download the release asset for that run:
   `https://github.com/AmerDje/codex/releases/download/recodex-<run_number>/codex.exe`
   (resumable: `curl.exe -sSL -C - -o codex.exe "<url>"`)
3. Put it in `C:\Users\hp\.recodex\codex.exe`.
4. Put the **helper binaries** next to it (see below).
5. Create the shim `C:\Users\hp\.cargo\bin\recodex.cmd`:
   ```
   @echo off
   "C:\Users\hp\.recodex\codex.exe" %*
   ```

`C:\Users\hp\.cargo\bin` is already on `PATH`, so `recodex` works from any directory.

### Helper binaries (required)
Codex spawns helper executables from its own directory. They must sit **next to
`codex.exe`** (and, for some routes, under a `codex-resources\` subfolder):

```
C:\Users\hp\.recodex\
  codex.exe
  codex-code-mode-host.exe
  codex-command-runner.exe
  codex-windows-sandbox-setup.exe
  codex-resources\
    codex-command-runner.exe
    codex-windows-sandbox-setup.exe
```

These are **not** built by the workflow (the code-mode host needs a V8 prebuilt that
isn't downloadable in CI). Copy them from an already-installed Codex of a compatible
version, e.g.:

```
%LOCALAPPDATA%\OpenAI\Codex\bin\<hash>\                  (flat: all three)
%USERPROFILE%\.codex\packages\app-server-daemon\releases\<ver>-x86_64-pc-windows-msvc\
    bin\codex-code-mode-host.exe
    codex-resources\codex-command-runner.exe
    codex-resources\codex-windows-sandbox-setup.exe
```

### Tools / prerequisites on this machine
- Rust `1.95.0` via rustup (`rust-toolchain.toml`), plus the Windows SDK
  (`winget install Microsoft.WindowsSDK.10.0.26100`) and VS 2022 MSVC for local builds.
- Building locally is heavy (RAM/disk); prefer the CI build.

---

## 2. Running `recodex`

```powershell
recodex                                  # interactive TUI
recodex exec "your prompt"               # non-interactive, one turn
recodex exec --json "your prompt"        # event stream as JSONL
recodex -m gpt-5.5                       # run with a specific model
recodex -s danger-full-access            # run without the Windows sandbox
recodex -C "C:\some\dir"                 # set the working root
recodex -c key=value                     # override a config value (dotted path)
recodex --enable <feature>               # enable a feature
recodex --disable <feature>              # disable a feature
recodex --version                        # prints "codex-cli <version>"
```

Useful subcommands: `exec`, `review`, `login`, `mcp`, `plugin`, `app-server`,
`doctor`, `sandbox`, `debug` (`debug prompt-input`, `debug models`).

`recodex` shares the same `CODEX_HOME` (`C:\Users\hp\.codex`) as `codex`, so it uses the
same auth (`auth.json`), config, and sessions. Nothing extra to log in.

---

## 3. New features added in this fork

### 3.1 `compact_context` tool — let the agent request compaction

Allows the **model** to ask for context compaction (summarize + slim the conversation)
at a safe point.

**Enable** (either):
- Per run: `recodex -c tools.compact_context.enabled=true`
- Permanently, in `~/.codex/config.toml`:
  ```toml
  [tools.compact_context]
  enabled = true
  ```

**Tool arguments** (all optional — the model may call it with `{}`):
| Arg | Required | Purpose |
|---|---|---|
| `reason` | no | Diagnostic only. One of: `task_boundary`, `context_drift`, `large_unrelated_history`, `before_new_plan`, `context_pressure`, `skill_requested`, `other` |
| `instruction` | no | Steers what the summary should preserve / omit / de-emphasize |

**Example prompts to trigger it:**
```
Call the compact_context tool right now with arguments {"reason":"task_boundary"}.
Then reply with exactly COMPACT_DONE.
```
For everyday use, add guidance to your instructions instead of prompting each time:
> "At a clear task boundary or before starting a new plan, call `compact_context`;
> include an `instruction` describing what to preserve."

**What happens:** the tool queues a one-shot compaction request. At the next safe
continuation the harness runs compaction (`ContextCompaction`) with
`CompactionReason::AgentRequested`, and (for local/remote summarization) appends the
`instruction` to the summarization prompt. The tool result is `Context compaction request accepted.`

**Verify it ran:** because `exec --json` does not print tool calls, check the session
rollout for `"name":"compact_context"` and a `ContextCompaction` item:
`C:\Users\hp\.codex\sessions\<date>\rollout-*.jsonl`

### 3.2 `background_task_resume` feature — auto-resume on background finish

When a **unified-exec** process that outlived its turn finishes while the thread is
**idle**, Codex starts an internal continuation turn so the model can act on it.

**Enable:**
- Per run: `recodex --enable background_task_resume`
- Permanently: `[features]` → `background_task_resume = true`

**Behaviour / caveats:**
- Only fires when the thread is idle (if a turn is active, the model already sees output
  via `write_stdin`). Skips Plan mode.
- One continuation turn **per process exit** — with many background processes you can get
  a burst of turns. Terminated processes (`/stop`) also count as "finished".
- Marked `UnderDevelopment`; default **off**. Recommended only for intentional
  "fire-and-forget long task" use, not as a daily default.

### 3.3 Version bump for model availability

Model access is gated by the client version. The bundled catalog
(`codex-rs/models-manager/models.json`) lists `minimal_client_version` per model, e.g.
`gpt-6.1-sol → 0.153.0`, `gpt-6-sol → 0.155.0`, `gpt-5.5 → 0.124.0`. The workspace
`version` in `codex-rs/Cargo.toml` was raised **`0.0.0` → `0.161.0`** so newer models are
accepted by the CLI.

---

## 4. Troubleshooting

### "The '<model>' model is not supported when using Codex with a ChatGPT account."
- **Cause:** the client version is below that model's `minimal_client_version`, or the
  model isn't offered to your account on this route (CLI vs app/daemon).
- **Fix:** confirm `recodex --version` is high enough (this fork reports `0.161.0`);
  or use a supported model, e.g. `-m gpt-5.5` (verified) or `gpt-5.6-sol`
  ("at capacity" is a transient load error, retry).

### "Code Mode is unavailable because … codex-code-mode-host.exe: host executable was not found"
### (or "the tool runtime cannot start")
- **Cause:** helper binaries are missing next to `codex.exe`.
- **Fix:** copy `codex-code-mode-host.exe`, `codex-command-runner.exe`,
  `codex-windows-sandbox-setup.exe` (and `codex-resources\`) into
  `C:\Users\hp\.recodex\` — see §1.

### "Failed to create unified exec process: helper_unknown_error: setup refresh had errors"
- **Cause:** the Windows sandbox setup can't update the ACL on
  `%LOCALAPPDATA%\OpenAI\Codex\runtimes\cua_node\…\bin\node_repl.exe` — `os error 32`
  (file in use). This is an upstream Codex bug (openai/codex issue **#52033**), not a
  `recodex` bug — the npm `codex` fails the same way.
- **Fix / workarounds:**
  1. Fully quit the Codex desktop app (or reboot) so its computer-use (CUA) runtime
     releases `node_repl.exe`, then retry.
  2. Otherwise run without the sandbox: `recodex -s danger-full-access`
     (verified working; less isolated).

### Copying `codex.exe` fails with "being used by another process"
- **Cause:** a running `recodex` holds the file.
- **Fix:** close all `recodex` sessions, then copy/overwrite.

### The release-asset download is slow / truncated
- **Fix:** use a resumable download and verify the byte size matches the asset:
  ```powershell
  curl.exe -sSL -C - -o codex.exe "https://github.com/AmerDje/codex/releases/download/recodex-<run_number>/codex.exe"
  ```

### Changing the version triggers a very long build
- Editing `codex-rs/Cargo.toml` `version` invalidates every crate → full rebuild on CI.

### Background terminals pile up / lose their ids after compaction
- Background processes are **session-scoped** and survive turns and compaction; the
  harness keeps them but the model may forget their session ids after a summary.
- Use **`/stop`** (TUI) to close them, or ask the agent to terminate the ones it started.

---

## 5. Updating `recodex`

1. Make changes on the `recodex` branch and push (or use **Run workflow** → `build-recodex`).
2. Wait for the run; a new release `recodex-<n>` is published.
3. Download `codex.exe` for that tag and replace `C:\Users\hp\.recodex\codex.exe`
   (close any running `recodex` first).
4. Keep the helper binaries present (see §1).
5. `recodex --version` to confirm.
