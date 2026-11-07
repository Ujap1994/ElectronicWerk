# Updates

ElectronicWerk tells you when a new version is available and shows what it
brings. It **never downloads or installs anything by itself**: you decide
whether and when to update, and you install the new version like the first
one.

## The update notice

A few seconds after start, ElectronicWerk asks GitHub for the newest release.
If it is newer than the installed version, this notice appears:

![The update notice](/screenshots/update-available-light.png)

| Part                                  | Meaning                                                                        |
| ------------------------------------- | ------------------------------------------------------------------------------ |
| **ElectronicWerk x.y.z is available** | The new version.                                                               |
| **Installed version · Released on**   | Your current version and the release date of the new one.                      |
| **What's new**                        | The release notes: new features, fixes and the download list for every system. |
| Installation hint                     | A reminder that updates are installed manually.                                |
| **Skip this version**                 | Do not announce this version again at startup (see below).                     |
| **Later**                             | Close the notice; it appears again at the next start.                          |
| **Open download page**                | Opens the release page in your browser, where you download the package.        |

![The update notice in the dark appearance](/screenshots/update-available-dark.png)

## Installing an update

1. Click **Open download page**.
2. Download the package for your system (the release notes contain a table of
   all files).
3. Quit ElectronicWerk and install the new version as described in
   [Installation](./installation). Your settings are kept.

## Skipping a version

Tick **Skip this version** and close the notice with **Later** or **Open
download page**. ElectronicWerk then stays quiet about this version at startup.
As soon as an even newer version appears, you are notified again.

A skipped version is shown in the [settings](./settings). Click **Show again**
to undo the decision. A manual check (below) always reports the newest version,
even a skipped one, with the checkbox already ticked.

## Checking manually

Use any of these:

- **ElectronicWerk → Check for Updates…** (macOS) or **Help → Check for
  Updates…** (Windows, Linux),
- **Check now** in the [settings](./settings),
- the [command palette](./command-palette) entry **Check for updates**.

A manual check always shows a result:

| Result           | Example                                                                    |
| ---------------- | -------------------------------------------------------------------------- |
| A newer version  | The update notice shown above                                              |
| No newer version | ![You already have the newest version](/screenshots/update-up-to-date.png) |
| The check failed | ![The update check failed](/screenshots/update-error.png)                  |

A failed check usually means that there is no internet connection or that
GitHub's limit of 60 requests per hour from your network has been reached. Try
again later.

## Turning the automatic check off

Clear **Check for updates at startup** in the [settings](./settings). Manual
checks keep working.

## Privacy

The update check sends one request to `api.github.com` that contains only the
user agent `ElectronicWerk/<version>`. ElectronicWerk sends no personal data, no
usage statistics and no identifiers. GitHub sees your IP address, as with any
web request.

## Why no automatic installation?

ElectronicWerk is open source and its packages are not signed with paid
certificates. Automatic installation of unsigned software would be blocked by
macOS and Windows, and silently replacing programs is not good practice. The
notice gives you the same convenience while you stay in control.
