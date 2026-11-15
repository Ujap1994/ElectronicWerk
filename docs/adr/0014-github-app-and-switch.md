# ADR-0014: A GitHub App for the sign-in, and GitHub switched off

- **Status:** accepted
- **Date:** 2026-11-15
- **Supersedes:** the sign-in part of [ADR-0011](./0011-github-integration.md)

## Context

ADR-0011 planned the browser sign-in through an **OAuth app**. Before the
maintainer registered it, they reviewed what such a sign-in gives
ElectronicWerk. The answer did not fit the security goals of the project:

- An OAuth app can only ask for the `repo` scope, which means read and write
  access to **every** public and private repository of the account and of
  its organizations, plus the right to create and delete them.
- Its tokens never expire. A token copied off a computer keeps working until
  someone revokes it by hand.

The maintainer also wants time to decide whether ElectronicWerk should talk
to GitHub at all. Until then, only projects on the computer should be
offered.

## Decision

### A GitHub App instead of an OAuth app

The browser sign-in (device flow) uses a **GitHub App** with the least
permissions that push and pull need:

| Permission (repository) | Access         | Why                                   |
| ----------------------- | -------------- | ------------------------------------- |
| Contents                | Read and write | Clone, pull and push the project      |
| Metadata                | Read           | Required by GitHub for every app      |
| everything else         | none           | No administration, issues, workflows… |

- **The user chooses the repositories.** A GitHub App works only on the
  repositories of its installation: the user installs it for their account
  on GitHub and picks _Only select repositories_ (or all). A user token of
  the app reaches only repositories that both the user and the installation
  may use. **Choose repositories…** in ElectronicWerk opens that page.
- **Short-lived tokens.** A user token of the app expires after 8 hours. It
  comes with a refresh token (valid for 6 months, usable once).
  ElectronicWerk renews the token 5 minutes before it expires, one renewal
  at a time: every renewal makes the previous token and refresh token
  invalid. A rejected or expired refresh token signs out. Without network, a
  token that has not expired yet keeps working.
- **No secret.** The device flow and the refresh of device-flow tokens need
  only the public client id. ElectronicWerk ships no client secret, and no
  server of the project stands between the app and GitHub.
- **No repositories are created by the app.** Creating repositories needs
  the _Administration_ permission, which also allows deleting them. Signed
  in with the app, the wizard and **Put on GitHub** therefore offer existing
  empty repositories only, with **New repository on GitHub…** (GitHub's
  _new repository_ page, the name filled in) and **Refresh**. The main
  process refuses to create a repository for an app sign-in as well.
- **Repositories of the app.** The list for **Open from GitHub** and for an
  existing empty repository comes from `GET /user/installations` and
  `GET /user/installations/{id}/repositories`, so it shows exactly what the
  app may use. Without an installation the error `notInstalled` explains
  what to do.
- **Personal access tokens stay** as the other way to sign in.
  **Create a token on GitHub** now opens the page for a _fine-grained_ token,
  pre-filled through GitHub's template URL (`contents=write`, 90 days);
  the user chooses its repositories there. Classic tokens still work.
- **Stored sign-in.** `github-token.bin` holds a JSON record (method, token,
  expiry, refresh token and its expiry), encrypted with `safeStorage` as
  before. A bare token written by 0.12 to 0.14 is read as a personal access
  token.

The app's client id and URL name are constants in `shared/github.ts`
(`GITHUB_APP_CLIENT_ID`, `GITHUB_APP_SLUG`); the browser sign-in is offered
only when both are set. `EW_GITHUB_CLIENT_ID` and `EW_GITHUB_APP_SLUG`
override them.

### GitHub switched off

The whole GitHub integration is behind a switch that is **off** by default:
it is on only when ElectronicWerk starts with the environment variable
`EW_ENABLE_GITHUB=1`.

- Off, the main process does not register a single `github:*` or `git:*`
  handler. Nothing in the app can sign in, read the stored token or call
  GitHub's API, whatever the user interface does.
- The user interface hides everything that needs it: **Open from GitHub** on
  the start screen and in the command palette, **Sign in to GitHub**, the
  storage choice of the new project wizard (new projects are created on the
  computer), **Source control** in the project tree and the toolbar, and the
  **GitHub** section of the settings.
- A token stored by an earlier version stays where it is (encrypted) and is
  neither read nor sent; **Sign out** after switching GitHub on deletes it.
- The update check and the download of the base library keep using GitHub's
  public release pages; they never send a token.

## Consequences

- When the maintainer decides to offer GitHub, they register the GitHub App
  (permissions above, device flow on, no callback, no webhook), set its
  client id and URL name, and switch the default on — a patch release.
- Users of the app sign-in create a repository on GitHub themselves; that is
  one more step than with the `repo` scope, and the price for not giving
  ElectronicWerk the right to delete repositories.
- The tests switch GitHub on (`EW_ENABLE_GITHUB=1`) and use the local
  stand-in for GitHub, which now issues expiring app tokens, renews them and
  knows the app's installation. An end-to-end test checks that GitHub is
  hidden and has no handlers when the switch is off.
