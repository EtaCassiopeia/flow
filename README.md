# flow

A personal Ratatui TUI that turns the daily ritual of "I just got assigned a
ticket → set up a worktree → start a tmux session → start coding" into one
keystroke, and just as quickly tears it down when the PR merges.

Inspired by [`lazygit`](https://github.com/jesseduffield/lazygit),
[`gitui`](https://github.com/extrawurst/gitui), and
[`giff`](https://github.com/bahdotsh/giff).

## What it does

A "work unit" = `(Jira ticket, repo, git worktree, tmux session)`. `flow` is a
single-binary remote control for the lifecycle of those work units.

| Flow | Keystrokes |
|---|---|
| Start a ticket | Dashboard → `t` → pick ticket → `c` → pick repo → `Enter` → drop straight into a tmux session in the new worktree |
| Manage worktrees | `w` → see every worktree on disk with branch / dirty / ahead-behind / attached-session badges → `Enter` to attach, `d` to delete (with branch-lifecycle prompt) |
| Review a PR | `p` → pick PR → `Enter` → fresh worktree + tmux session checked out at the PR head |

The conventions baked in:

- **Worktree path:** `<code_root>/<org>/<repo>-<TICKET>` (sibling of the main
  clone). Branch name: `<TICKET>-<title-slug>`. Tmux session name: `<TICKET>`.
- **Where things live:** config at `~/.config/flow/config.toml`, cache at
  `~/.cache/flow/snapshot.json`, secrets in macOS Keychain (service `flow`).

## Requirements

- macOS (the only supported platform today — secrets live in the Keychain
  via the `keyring` crate's `apple-native` backend).
- **Rust 1.85+** (edition 2024).
- **git** ≥ 2.30 — `git worktree` is the central primitive.
- **tmux** ≥ 3 — sessions and `switch-client`.
- **gh** (recommended) — already on PATH and authenticated. Without it
  `flow` falls back to a personal-token path via `octocrab` (see
  [GitHub access](#github-access)).
- A Jira Cloud account with an API token, if you want the Tickets screen
  to do anything useful.

## Build & install

```sh
git clone <repo>
cd my-workflow
cargo build --release
cp target/release/flow /usr/local/bin/   # or anywhere on PATH
```

There's no `cargo install` story yet because the project hasn't been
published; copy the binary or run from `target/release/flow` directly.

## First run

Two paths.

### Option A — first-run wizard

If `~/.config/flow/config.toml` is missing, launching `flow` drops you
straight into a setup screen. It asks for:

| Field | What goes there |
|---|---|
| `code root` | Parent directory for clones, e.g. `~/code`. Org-grouped: `<code_root>/<org>/<repo>` |
| `jira host` | `<your-org>.atlassian.net` |
| `jira email` | The email tied to your Atlassian account |
| `jira token` | An API token from <https://id.atlassian.com/manage-profile/security/api-tokens> — stored in Keychain, never in the config file |
| `github token` | Optional. Only needed if `gh` is *not* authenticated on this machine. Stored in Keychain. |

Tab/Shift-Tab between fields, `Enter` on `[ submit ]` writes
`config.toml`, saves the tokens to the Keychain, and drops you into the
Dashboard.

### Option B — write the config by hand

Drop a file at `~/.config/flow/config.toml`:

```toml
code_root   = "~/code"
default_org = "pfc"

[jira]
host        = "company.atlassian.net"
email       = "you@example.com"
jql_my_open = 'assignee = currentUser() AND statusCategory != Done ORDER BY updated DESC'

[github]
auto_discover = true
watched       = ["pfc/pfc-ledger"]   # optional explicit allowlist

[tmux]
session_template       = "{ticket}"   # vars: {ticket} {repo} {slug}
kill_on_remove_worktree = true

[ui]
theme                 = "dark"
refresh_interval_secs = 0             # 0 = manual refresh only
```

Then save tokens to the Keychain (you can use the wizard, or `security`
on the command line):

```sh
security add-generic-password -s flow -a "jira:you@example.com" -w "<jira-api-token>"
# only if `gh` isn't authenticated:
security add-generic-password -s flow -a "github" -w "<github-pat>"
```

### Verifying the setup

```sh
flow doctor
```

Sample output:

```
flow doctor
  config path : /Users/you/.config/flow/config.toml
  config found: true
  jira host   : company.atlassian.net
  jira email  : you@example.com
  code root   : ~/code
  jira token  : present
  github token: missing
  git         : git version 2.50.1
  tmux        : tmux 3.6a
  gh          : gh version 2.92.0
  github      : backend = gh
```

`backend = gh` means `flow` will shell out to `gh` for repo and PR data;
`octocrab` means it'll use the saved Keychain token; `disabled` means
GitHub-touching screens will surface a toast asking you to fix one of
the two.

## Usage

### Keybindings

Most of this is also reachable from `?` inside the TUI.

**Global**

| Key | Action |
|---|---|
| `?` | Toggle help overlay |
| `q` | Quit |
| `Esc` | Back / cancel |
| `R` | Force-refresh everything (bypass cache) |

**Dashboard**

| Key | Action |
|---|---|
| `t` | Tickets |
| `w` | Worktrees |
| `p` | PRs |
| `r` | Force-refresh tickets |

**Lists (Tickets / Worktrees / PRs)**

| Key | Action |
|---|---|
| `j` / `k` | Move |
| `/` | Filter (Tickets) |
| `Enter` | Activate (open detail / attach session / checkout PR) |
| `o` | Open URL in browser |
| `r` | Force-refresh current view |

**Worktree detail**

| Key | Action |
|---|---|
| `d` | Delete (opens the branch-lifecycle prompt) |
| `m` | Cycle delete mode: keep / delete-if-merged / force-delete |

### Try it without credentials

```sh
flow --mock
```

Boots with seeded `MockJira` and `MockGithub` clients (3 tickets, 2
repos, 1 PR). Every screen renders, but creating a worktree will fail
because the seed repos don't exist on disk — handy for working on the
TUI itself without touching real services.

### A real walkthrough

1. `flow` → Dashboard renders instantly with whatever was in the cache
   from your last run; a background refresh fires.
2. `t` → Tickets list. `j`/`k` to navigate, `/` to filter by key or
   summary text.
3. `Enter` on a ticket → detail screen with the watched repos.
4. `c` → Confirm screen. Shows the resolved plan: worktree path, branch
   name, session name. `Enter` to execute.
5. `flow` runs `git worktree add` + `tmux new-session -d` + hands the
   terminal over to `tmux attach`. You're now inside the new session in
   the new worktree on the new branch.
6. Detach with `prefix-d`. `flow` resumes its TUI on the Worktrees
   screen with the new entry visible.
7. When the PR merges: `w` → `d` → cycle to "delete if merged" → `Enter`.
   Worktree, branch, and tmux session are torn down.

### Inside an existing tmux session

If `flow` is launched from inside tmux (i.e. `$TMUX` is set), it uses
`tmux switch-client -t <name>` instead of `tmux attach`. The TUI doesn't
need to leave the alternate screen; tmux performs the swap and `flow`
resumes when you switch back.

## How it works (in 60 seconds)

- **Architecture.** Component-based Ratatui app. `Screen` is an enum of
  per-screen states; the event loop owns one of these and dispatches
  key events to per-screen handlers. Single `tokio::mpsc` carries
  input, fetch results, ticks, and command requests.
- **Cache.** Tickets, repos, worktrees, and per-repo PRs are cached
  with TTLs (5 min / 24 h / 30 s / 2 min) and persisted to
  `~/.cache/flow/snapshot.json` on shutdown. On startup the snapshot is
  loaded, screens render the stale data instantly, and a background
  refresh swaps in fresh data when ready.
- **Cancellation.** Each fetch issues a `(FetchId, CancellationToken)`
  pair from a `Registry`. Force-refresh (`r` / `R`) cancels the
  in-flight fetch of the same kind before issuing a new one. Late
  results from cancelled fetches are dropped silently.
- **Tmux handover.** `Tui::suspend_for` leaves the alternate screen and
  raw mode, runs `tmux attach` synchronously with inherited stdio,
  re-enters alt-screen / raw mode on return. A panic hook runs the
  same teardown so a crash never wrecks the terminal.
- **GitHub backends.** At startup `services::github::detect()` probes
  `gh auth status`; if it succeeds, `flow` shells out to `gh` (handles
  fork PRs natively). Otherwise it reads the GitHub token from the
  Keychain and uses `octocrab`; PR checkout is replicated with `git
  fetch origin pull/<N>/head` + `git reset --hard FETCH_HEAD`. If
  neither path is available, every GitHub call returns a clear
  "how to fix" error that surfaces as a toast.

## Limitations

- macOS only (Keychain backend is hard-wired).
- The Repos screen is not yet implemented — repo data is loaded but not
  browseable as its own screen. Use the repo picker on TicketDetail.
- No auto-poll. `refresh_interval_secs = 0` is the only supported value
  right now.
- `flow doctor` makes a network call (`gh auth status`); offline it
  reports `disabled` even if `gh` is fine.

## Development

```sh
cargo fmt
cargo clippy --all-targets
cargo test                     # 25 unit tests, ~0.1s
cargo test -- --ignored        # tmux integration test (needs tmux on PATH)
```

The integration test pins tmux to a private socket (`tmux -L flow-test`)
so it doesn't touch your real sessions. The git tests use `tempfile`
and a real `git init` — they're hermetic.

Run the doctor subcommand whenever something feels off:

```sh
flow doctor
```

## Layout

```
src/
├── main.rs              # CLI parse, runtime boot, panic hook
├── app.rs               # event loop, command dispatch, fetch orchestration
├── error.rs             # top-level Error (thiserror)
├── msg.rs               # AppMsg, Command, FetchKind, FetchResult
├── fixtures.rs          # seed data for --mock
├── config/              # config TOML schema and load/save
├── domain/              # pure data types (Ticket, Repo, Worktree, Pr, …)
├── services/
│   ├── git.rs           # worktree add/remove/list/status
│   ├── tmux.rs          # has/new/attach/switch/kill
│   ├── jira.rs          # JiraClient trait + reqwest impl + mock
│   ├── github.rs        # GithubClient trait + gh impl + octocrab impl + mock
│   ├── keychain.rs      # macOS Keychain wrapper
│   └── slug.rs          # title → kebab-slug
├── cache/               # CacheEntry, Snapshot, load/save
├── runtime/             # cancellation Registry
└── ui/
    ├── mod.rs           # Tui (alt-screen + suspend/resume)
    ├── theme.rs
    ├── widgets/         # status_bar, spinner, toast, key_hint
    └── screens/         # dashboard, tickets, ticket_detail, confirm_create,
                         # worktrees, prs, setup, help
```
