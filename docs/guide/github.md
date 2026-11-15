# GitHub and source control

::: warning Switched off in this version
The GitHub integration is **switched off** while its sign-in is being
reviewed: ElectronicWerk offers only projects on this computer. The start
screen has no **Open from GitHub**, the new project wizard creates every
project on this computer, and **Source control**, **Sign in to GitHub** and
the **GitHub** section of the settings are hidden. Nothing in ElectronicWerk
signs in to GitHub or reads a stored GitHub token then.

To try the integration, start ElectronicWerk from a terminal with the
environment variable `EW_ENABLE_GITHUB=1`:

- macOS: `EW_ENABLE_GITHUB=1 /Applications/ElectronicWerk.app/Contents/MacOS/ElectronicWerk`
- Linux: `EW_ENABLE_GITHUB=1 electronicwerk` (or the path of the AppImage)
- Windows (PowerShell): `$env:EW_ENABLE_GITHUB = "1"`, then start
  `ElectronicWerk.exe` from the same window

The rest of this page describes the integration as it works then.

The [update check](./updates) and the download of the
[base library](./library) are not affected: they read GitHub's public
release pages without an account.
:::

With the integration switched on, ElectronicWerk keeps projects in **git**
and works with **GitHub** directly: put a project on GitHub, commit your
changes, push and pull them, work on branches, look at the history and open
projects from GitHub on another computer. You do not need to install git;
ElectronicWerk brings its own.

![Source control with two changed files](/screenshots/github-source-control.png)

The [project format](../reference/file-format) is made for this: every
document is a text file with a stable order, so commits stay small and the
history shows what changed.

## What ElectronicWerk can access

ElectronicWerk asks GitHub for as little as possible:

- **Only the repositories you choose.** Signed in with the browser,
  ElectronicWerk works through the _ElectronicWerk_ GitHub App. You decide on
  GitHub which repositories it may use (_Only select repositories_, or all).
  Every other repository of your account or your organizations stays out of
  reach.
- **Only their files.** The app may read and write the **contents** of these
  repositories (to clone, pull and push) and read their basic data. It may
  not create or delete repositories, change settings, or read issues, pull
  requests, secrets or workflows.
- **Short-lived tokens.** The token of the browser sign-in expires after
  **8 hours**. ElectronicWerk renews it by itself shortly before (with a
  refresh token that is valid for 6 months and works only once), so a token
  copied off your computer is soon worthless.
- **No server in between, no secret.** ElectronicWerk talks to GitHub
  directly. Nobody else — not the makers of ElectronicWerk either — sees
  your token or your repositories. The app's id built into ElectronicWerk is
  public and grants nothing by itself.

## Signing in to GitHub

Pushing, pulling and opening projects from GitHub need your GitHub account.
Open the [settings](./settings) and choose **Sign in to GitHub…** in the
**GitHub** section, or run **Sign in to GitHub** from the
[command palette](./command-palette). The wizard and the source control view
offer the same button when they need it.

![Signing in with the browser](/screenshots/github-sign-in.png)

There are two ways to sign in:

- **With your browser** (recommended). Choose **Sign in with the browser**.
  ElectronicWerk shows a code. **Copy code** copies it, **Open GitHub** opens
  `github.com/login/device` in your browser. Enter the code there and
  confirm. The dialog closes by itself once GitHub confirms. You never enter
  your password in ElectronicWerk. This option is shown when the
  ElectronicWerk GitHub App is registered for your version; otherwise use a
  token.
- **With a personal access token.** **Create a token on GitHub** opens
  GitHub's page for a new **fine-grained token**, filled in for
  ElectronicWerk: the permission _Contents: Read and write_ and 90 days of
  validity. Choose the repositories the token may use (_Only select
  repositories_), create it, paste it into **Token** and choose **Sign in**.
  ElectronicWerk checks the token with GitHub before it keeps it. Classic
  tokens with the `repo` scope work as well, but reach all your
  repositories.

### Choosing the repositories for ElectronicWerk

After the browser sign-in, ElectronicWerk may not use any repository yet.
**Choose repositories…** (in the settings, the wizard and **Open from
GitHub**) opens the app's page on GitHub:

1. The first time, GitHub asks where to install the ElectronicWerk app:
   choose your account (or an organization you manage).
2. Choose **Only select repositories** and pick the repositories with your
   projects, or **All repositories**.
3. Save, go back to ElectronicWerk and choose **Refresh**.

You can change the choice there at any time: the repositories you remove are
out of reach for ElectronicWerk at once.

### Where the token is kept

![The GitHub section of the settings](/screenshots/github-settings.png)

The settings show how you are signed in (_Signed in with the ElectronicWerk
app_ or _Signed in with a personal access token_). The token is encrypted by
the operating system and stored in the ElectronicWerk user folder
(`github-token.bin`), together with its expiry and the refresh token:

| System  | Protection                                                              |
| ------- | ----------------------------------------------------------------------- |
| macOS   | Keychain                                                                |
| Windows | Data Protection API (bound to your Windows account)                     |
| Linux   | Secret Service (GNOME Keyring, KWallet) through the desktop environment |

The settings also show how the token is protected. On Linux without a
keyring, the token would only be obfuscated: the settings then show a
warning. Install and unlock a keyring to protect it. If the system offers no
encryption at all, ElectronicWerk keeps the token only in memory until it
closes.

**Sign out** deletes the token from this computer. To revoke it for good,
revoke it on GitHub as well (_Settings → Applications → Authorized GitHub
Apps_ for the browser sign-in, _Settings → Developer settings → Personal
access tokens_ for a token). If you do not use ElectronicWerk for 6 months,
the refresh token expires and you sign in again.

The token is sent only to GitHub (`github.com` and `api.github.com`), never
to another server, even if a project's remote points elsewhere.

## A new project on GitHub

In the [new project wizard](./projects#creating-a-project), choose **On
GitHub** under _Where should the project live?_.

Signed in with the **browser** (the ElectronicWerk app), the project goes
into an **empty repository** you created on GitHub, because the app may not
create repositories:

![The wizard with an empty GitHub repository chosen](/screenshots/wizard-github.png)

1. **New repository on GitHub…** opens GitHub's _new repository_ page with
   the project name filled in and _Private_ chosen. Create it **without** a
   README, license or `.gitignore`, so that it stays empty.
2. **Choose repositories…** lets ElectronicWerk use it (not needed when the
   app may use all your repositories).
3. **Refresh** and choose it under **Repository**. The list shows the empty
   repositories the app may use.

Signed in with a **personal access token**, you can also let ElectronicWerk
create the repository:

| Choice                        | Meaning                                                                                                                                                                                                                                                                                      |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **New repository**            | ElectronicWerk creates the repository in your account. **Repository name** starts as the project name with spaces replaced by `-`; letters, digits, `.`, `-` and `_` are allowed. A fine-grained token needs the permission _Administration: Read and write_ (on all repositories) for this. |
| **Description**               | Optional; shown on GitHub and in the README.                                                                                                                                                                                                                                                 |
| **Private repository**        | Checked by default: only you and the people you invite can see it.                                                                                                                                                                                                                           |
| **Existing empty repository** | A repository you created on GitHub without any files (no README, no license). The list shows the empty repositories the token may use.                                                                                                                                                       |

**Create project** creates the project on this computer as usual (in the
location of the wizard), then:

1. makes the project folder a git repository (branch `main`, or the default
   branch of the existing repository),
2. adds a `README.md` with the project name, the description and how to open
   the project,
3. commits everything as **Create _project name_**,
4. creates the repository (with a token), or uses the empty one, and pushes
   the commit.

The `.gitignore` of every project keeps local state (`.ewerk/`: autosave
journal, caches) and generated outputs (`outputs/generated/`) out of the
repository.

If GitHub cannot be reached, the project is still created on this computer
and a notice says why it was not put on GitHub. You can do it later from the
source control view.

## The source control view

Every project has **Source control** at the end of the
[project tree](./project-tree), and the **Source control** button in the
toolbar opens it too. The button shows how many files changed since the last
commit:

![The source control button with two changed files](/screenshots/github-toolbar.png)

The view shows, from top to bottom:

- the **repository** on GitHub, with **Open on GitHub**,
- the **branch** and how it compares with GitHub: _Up to date with GitHub_,
  _Commits to push: 2_, _Commits to pull: 1_, or _This branch is not on GitHub
  yet_,
- **Check GitHub**, **Pull** and **Push**,
- the **changes** since the last commit, the **commit message** and
  **Commit** / **Commit and push**,
- the **history** of the branch: message, author, time and the short commit
  id.

### A project that is not on GitHub yet

For a project created **on this computer** (or imported), the view offers
**Put the project on GitHub** with the same choices as the wizard (an
existing empty repository, or with a token also a new one). **Put on
GitHub** does the same steps as the wizard. Save the project first.

A project folder that is already a git repository without GitHub (for
example one you made with git yourself) shows _A local git repository without
GitHub_: you can commit and switch branches; push and pull need a GitHub
remote.

### Committing

A commit records the **saved** files of the project folder. Unsaved changes
are not part of it: if the project has unsaved changes, the view says so and
offers **Save**.

1. Check the files to commit. All changed files are checked; uncheck the ones
   that should wait. Each file is marked **new**, **changed** or **deleted**.
2. Write a **commit message**: what you changed and why, for example
   _Route the power section_.
3. Choose **Commit** to record it on this computer, or **Commit and push** to
   send it to GitHub right away.

Commits carry your name and your private GitHub e-mail address
(`id+login@users.noreply.github.com`), so your real e-mail address is never
published. If the repository has its own `user.name` and `user.email` (set
with git), those are used instead.

The view reads the state of the folder again when you save, when files are
added or removed, and when you come back to the window. Files changed by
other programs (a datasheet, notes) show up as changes too.

### Pushing and pulling

- **Push** sends your commits to GitHub. If GitHub has commits you do not
  have yet, GitHub refuses the push: pull first, then push again.
- **Check GitHub** asks GitHub for new commits without changing anything here.
  The state line then shows how many commits there are to pull.
- **Pull** takes the new commits from GitHub. If you have no commits of your
  own, the branch simply moves forward. If both sides have new commits that
  changed **different files**, ElectronicWerk merges them with a merge
  commit. Afterwards the project is read again from disk, and all open tabs
  close.

Pull needs a clean project: save it and commit (or remove) your changes
first.

### Conflicts

ElectronicWerk never merges changes **inside** a file. A schematic or a board
is more than lines of text: two edits of the same sheet merged line by line
could result in a broken design. If you and GitHub changed the same file,
**Pull** stops before it changes anything and lists the files:

![A pull that found the same file changed on both sides](/screenshots/github-conflict.png)

Your work is safe: nothing was changed. To keep both versions:

1. Create a **new branch** (see below). Your commits come along.
2. **Push** the new branch.
3. Combine the two versions by hand, for example on two computers or by
   opening the other branch in a copy of the project, and commit the result.

A visual comparison of schematic and board revisions is planned for a later
version.

### Branches

The **Branch** list shows the branches of this computer and, marked
_(GitHub)_, the branches that exist only on GitHub.

- Choose a branch to **switch** to it. The project is read again with the
  files of that branch. Save and commit your changes first.
- Choose a GitHub branch to get it onto this computer and switch to it.
- **New branch…** creates a branch at the current commit and switches to it.
  Your files do not change; new commits go to the new branch.

![Creating a branch](/screenshots/github-new-branch.png)

A new branch is on this computer only until you **push** it.

Branch names may contain letters, digits, `-`, `_`, `.` and `/` (for example
`rev-b` or `feature/usb-c`), but no spaces and no `..`.

## Opening a project from GitHub

On the start screen, choose **Open from GitHub** (or run **Open from GitHub**
from the command palette):

![Open from GitHub](/screenshots/github-clone.png)

1. Choose a repository. **Filter** finds it by name or description. The list
   shows the repositories you may push to, newest first, with **Private** for
   private ones: signed in with the browser, the repositories you chose for
   the ElectronicWerk app; with a token, the repositories of the token.
   _Missing a repository?_ **Choose repositories…** lets the app use it,
   then **Refresh**.
2. **Into folder** is where the project folder goes: `<folder>/<repository
name>`. The folder must not exist yet, or it must be empty.
3. **Download and open** clones the repository and opens its project (the
   `.ewproj` file in the top folder of the repository).

The cloned project remembers its repository: commit, push and pull work right
away.

## What is not supported yet

- Merging changes inside a file (see [Conflicts](#conflicts)), and a visual
  comparison of revisions.
- Remotes other than GitHub for push and pull (a local git repository works
  for commits and branches).
- Pull requests and issues: use GitHub's website for them (**Open on
  GitHub**).
- Git LFS: large files are stored in the repository as they are.

## Troubleshooting

| Problem                                                   | Solution                                                                                                                                                                                                                                |
| --------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| _GitHub rejected the request_                             | The token expired or was revoked, or it has no access to the repository. Sign out and sign in again; for a fine-grained token, give it access to the repository.                                                                        |
| _ElectronicWerk may not use any of your repositories yet_ | Signed in with the browser, the app has no repositories yet: **Choose repositories…**, pick them on GitHub, then **Refresh**.                                                                                                           |
| _Sign in to GitHub first_ after a long break              | The sign-in of the app ends when it was not used for 6 months, or when you revoked it on GitHub. Sign in again.                                                                                                                         |
| _GitHub could not be reached_                             | Check the internet connection. ElectronicWerk uses the proxy settings of the system.                                                                                                                                                    |
| _GitHub has newer commits. Pull them first…_              | Someone pushed in the meantime. **Pull**, then **Push** again.                                                                                                                                                                          |
| _Commit or undo your changes first._                      | Pull and switching branches need a clean project. Commit the listed files, or remove the ones you do not need.                                                                                                                          |
| _The folder or repository is not empty._                  | The wizard and **Put on GitHub** need an empty repository: choose a new name or another repository. To get a repository with content, use **Open from GitHub**. For **Open from GitHub**, choose another folder or remove the old copy. |
| _This repository has no ElectronicWerk project_           | The repository has no `.ewproj` file in its top folder.                                                                                                                                                                                 |
| A warning about the token on Linux                        | No keyring is running. Install GNOME Keyring or KWallet and make sure it is unlocked when you sign in.                                                                                                                                  |
