# ADR-0011: GitHub integration and source control

- **Status:** accepted
- **Date:** 2026-11-14
- **Milestone:** M11

## Context

M11 connects projects with GitHub. The roadmap asks for:

- a sign-in with the OAuth **device flow** (no secret in the app), or a
  personal access token, with the token in the **OS keychain**;
- **Local** or **GitHub** in the new-project wizard: a new repository
  (public or private, with `.gitignore`, README and the project) or an
  existing one;
- clone/open from GitHub, commit and push from the project tree, pull,
  branch switch and a history view; a visual diff later.

The constraints of the earlier milestones hold: the renderer is sandboxed
and only talks to the main process through a small, checked IPC API
(ADR-0001); the project files are written by the engine, which keeps them
in memory while a project is open (ADR-0002); end-to-end tests never contact
the internet.

Users of ElectronicWerk are hobbyists and engineers, not necessarily git
users. Many have no git installed (most Windows machines), and a project
must never end up half merged.

## Decision

### Git in the main process with isomorphic-git

Git runs in the **main process** with
[isomorphic-git](https://isomorphic-git.org/) (MIT), a git implementation in
JavaScript:

- No git installation is needed on any platform, and the version is fixed.
- The repository on disk is a normal git repository: the user can work on it
  with any other git tool as well.
- isomorphic-git's HTTP client is replaced by a small adapter on Electron's
  `net.fetch`, so git uses the system's proxy settings and certificate store
  like the browser.

The alternatives were the system's git (not present everywhere, versions and
configuration differ, credential helpers would have to be driven) and
libgit2 bindings (`nodegit`: native builds per platform, poorly maintained).
Doing git in the Rust engine (`gix`) was considered: it would make git
available in the CLI too, but `gix` cannot push over HTTP yet. The CLI can
use the user's own git meanwhile.

The renderer never names a folder: every git action takes the manifest path
of the project, and the main process accepts it only if it is the project
open in the main window. Clones go into a folder the user chose.

### Merging: whole files only

isomorphic-git can merge text files line by line. A schematic or a board
merged line by line can be valid JSON and still a broken design (a wire
without its junction, a footprint referencing a removed pad). ElectronicWerk
therefore merges **commits, not files**: a pull fast-forwards, or creates a
merge commit when both sides changed **different** files. A file changed on
both sides is a **conflict**: the pull stops before it changes anything and
lists the files (a custom merge driver reports every file changed on both
sides as a conflict; identical changes are no conflict). A pull also needs a
clean working tree, so it can never overwrite unsaved or uncommitted work.

A semantic merge (per symbol, per track) and the visual diff are later work;
they build on the engine's document model, not on git.

### After a pull or a branch switch

The engine holds the open project in memory. After git changed the files,
the UI closes the project in the engine and opens it again (it must be
saved first), and the workspace closes its tabs, because documents may have
changed or disappeared.

### Sign-in and token

- **Device flow** (RFC 8628) with ElectronicWerk's GitHub OAuth app: the app
  only knows the public client id; the user confirms a code on github.com.
  The browser sign-in is offered when a client id is configured (the
  constant in `shared/github.ts`, or `EW_GITHUB_CLIENT_ID`).
- **Personal access tokens** as the alternative (classic with `repo` and
  `read:user`, or fine-grained). The token is checked with `GET /user`
  before it is kept.
- The token is encrypted with Electron's `safeStorage` (macOS Keychain,
  Windows DPAPI, Linux Secret Service) in `github-token.bin` in the user
  folder. Without a keyring Linux falls back to a fixed key (`basic_text`):
  the settings show a warning. Without encryption the token stays in memory
  only.
- The token is handed to git only for URLs on GitHub's host, never to
  another remote, and never reaches the renderer.
- Commits are signed with the account's name and its private no-reply
  address (`id+login@users.noreply.github.com`), unless the repository has
  its own `user.name` and `user.email`.

### Publishing a project

A new project on GitHub is first created locally by the engine as usual
(with its `.gitignore`, which keeps `.ewerk/` and `outputs/generated/` out),
then the main process initializes the repository, writes a README, commits,
creates the repository (`POST /user/repos`, without auto-init) or checks
that the chosen one is empty, and pushes. The same steps put an existing
local project on GitHub.

### Tests

A local stand-in for GitHub (`apps/desktop/test/github-server.ts`) serves
the REST API, the device flow and git over HTTP through the system git's
`git http-backend` (part of git on all CI runners). Unit tests run the whole
flow against it (sign-in, publish, commit, push, clone, fast-forward, merge,
conflict, branches, token only to GitHub), and the end-to-end tests drive
the UI against it. The app reads the server's addresses from test hooks
(`EW_GITHUB_API`, `EW_GITHUB_WEB`) and uses a fake keychain in tests.

## Consequences

- Git works out of the box on every platform; the repositories are standard.
- Conflicting edits of the same file need manual work (a new branch, then
  combining by hand) until the semantic merge exists. The user's work is
  never changed by a failed pull.
- isomorphic-git is slower than native git on very large repositories
  (thousands of files, long histories); projects are far below that.
- Git LFS, SSH remotes and other hosts than GitHub for push and pull are not
  supported yet.
