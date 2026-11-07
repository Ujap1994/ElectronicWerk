# Installation

ElectronicWerk runs on Windows, macOS and Linux, on both x64 (Intel/AMD) and
arm64 (Apple Silicon, Snapdragon, Raspberry Pi 5 class) processors. Download
the package for your system from the
[latest release](https://github.com/Ujap1994/ElectronicWerk/releases/latest).

## Which file do I need?

| System                                    | File (example for version 0.1.0)                             |
| ----------------------------------------- | ------------------------------------------------------------ |
| Windows, installer                        | `ElectronicWerk-0.1.0-win-x64-setup.exe` (or `arm64`)        |
| Windows, portable (no installation)       | `ElectronicWerk-0.1.0-win-x64-portable.exe` (or `arm64`)     |
| macOS, any Mac                            | `ElectronicWerk-0.1.0-mac-universal.dmg`                     |
| macOS, smaller download                   | `…-mac-arm64.dmg` (Apple Silicon) or `…-mac-x64.dmg` (Intel) |
| Ubuntu, Debian, Linux Mint, Pop!\_OS      | `ElectronicWerk-0.1.0-linux-x64.deb` (or `arm64`)            |
| Fedora, openSUSE, RHEL, Rocky, Alma       | `ElectronicWerk-0.1.0-linux-x64.rpm` (or `arm64`)            |
| Arch Linux, CachyOS, Manjaro, EndeavourOS | `ElectronicWerk-0.1.0-linux-x64.pacman` (or `arm64`)         |
| Any distribution, Flatpak                 | `ElectronicWerk-0.1.0-linux-x64.flatpak` (or `arm64`)        |
| Any distribution, Snap                    | `ElectronicWerk-0.1.0-linux-x64.snap` (or `arm64`)           |
| Any distribution, single file             | `ElectronicWerk-0.1.0-linux-x64.AppImage` (or `arm64`)       |
| Any distribution, archive                 | `ElectronicWerk-0.1.0-linux-x64.tar.gz` (or `arm64`)         |

::: tip Not sure which processor you have?

- **Windows:** Settings → System → About → _System type_.
- **macOS:** Apple menu → About This Mac. "Apple M…" means Apple Silicon
  (arm64). If unsure, take the **universal** package; it runs on every Mac.
- **Linux:** run `uname -m` in a terminal. `x86_64` means x64, `aarch64`
  means arm64.

:::

## Why does my system warn me?

ElectronicWerk is open source and **not code-signed** with a paid certificate.
Windows and macOS therefore warn you the first time you open it. The steps
below explain how to confirm that you want to open it. If you want to be sure
the file is genuine, [verify the checksum](#verify-the-download).

## Windows

### Installer

1. Double-click `ElectronicWerk-<version>-win-<arch>-setup.exe`.
2. If **Microsoft Defender SmartScreen** shows "Windows protected your PC",
   click **More info**, then **Run anyway**.
3. Choose whether to install for your user only (default, no administrator
   rights needed) and pick the folder.
4. Start ElectronicWerk from the Start menu or the desktop shortcut.

### Portable version

The portable `.exe` runs without installation, for example from a USB stick.
Double-click it; it unpacks itself to a temporary folder and starts. Settings
are still stored in your user profile (see [Where are my settings
stored?](#where-are-my-settings-stored)).

## macOS

1. Open the downloaded `.dmg` file.
2. Drag **ElectronicWerk** onto the **Applications** folder.
3. Open ElectronicWerk from the Applications folder.
4. macOS says that it cannot check the app for malicious software. Click
   **Done** (or **Cancel**).
5. Open **System Settings → Privacy & Security**, scroll to the message about
   ElectronicWerk and click **Open Anyway**. Confirm with your password.
6. From now on, ElectronicWerk starts normally.

::: details Alternative: Terminal
If you prefer the terminal, remove the download quarantine flag once:

```sh
xattr -dr com.apple.quarantine /Applications/ElectronicWerk.app
```

:::

## Linux

### Debian, Ubuntu, Linux Mint (`.deb`)

```sh
sudo apt install ./ElectronicWerk-0.1.0-linux-x64.deb
```

### Fedora, openSUSE, RHEL (`.rpm`)

```sh
sudo dnf install ./ElectronicWerk-0.1.0-linux-x64.rpm      # Fedora, RHEL
sudo zypper install ./ElectronicWerk-0.1.0-linux-x64.rpm   # openSUSE
```

### Arch Linux, CachyOS, Manjaro (`.pacman`)

```sh
sudo pacman -U ./ElectronicWerk-0.1.0-linux-x64.pacman
```

### Flatpak

Flatpak needs the Flathub repository for the shared runtime:

```sh
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user ./ElectronicWerk-0.1.0-linux-x64.flatpak
flatpak run io.github.ujap1994.electronicwerk
```

### Snap

The snap is not in the Snap Store yet, so install it as a local file:

```sh
sudo snap install --dangerous ./ElectronicWerk-0.1.0-linux-x64.snap
```

### AppImage

```sh
chmod +x ElectronicWerk-0.1.0-linux-x64.AppImage
./ElectronicWerk-0.1.0-linux-x64.AppImage
```

::: warning Ubuntu 24.04 and newer
Recent Ubuntu versions restrict the sandbox that Electron-based AppImages
need, and the AppImage may not start. Use the `.deb` package instead, which
sets the sandbox up correctly.
:::

### Archive (`.tar.gz`)

Unpack the archive anywhere and run the `electronicwerk` program inside it.

## Verify the download

Every release contains `SHA256SUMS.txt`. Compare the checksum of your file with
the matching line:

::: code-group

```sh [macOS]
shasum -a 256 ElectronicWerk-0.1.0-mac-universal.dmg
```

```sh [Linux]
sha256sum ElectronicWerk-0.1.0-linux-x64.deb
```

```powershell [Windows]
Get-FileHash .\ElectronicWerk-0.1.0-win-x64-setup.exe -Algorithm SHA256
```

:::

## Command-line tool

The `ewerk` command-line tool is a separate download
(`ewerk-<version>-<system>-<arch>.tar.gz` or `.zip` on Windows). Unpack it and
put the `ewerk` program somewhere on your `PATH`. See
[Command-line tool](../reference/cli).

## Where are my settings stored?

| System  | Folder                                         |
| ------- | ---------------------------------------------- |
| Windows | `%APPDATA%\ElectronicWerk`                     |
| macOS   | `~/Library/Application Support/ElectronicWerk` |
| Linux   | `~/.config/ElectronicWerk`                     |

The settings are in `settings.json` in that folder. Deleting the file resets
every setting to its default.

## Uninstalling

- **Windows:** Settings → Apps → Installed apps → ElectronicWerk → Uninstall.
  The portable version only needs to be deleted.
- **macOS:** move ElectronicWerk from the Applications folder to the Trash.
- **Linux:** remove the package with your package manager, for example
  `sudo apt remove electronicwerk`, `sudo dnf remove electronicwerk`,
  `sudo pacman -R electronicwerk`, `flatpak uninstall io.github.ujap1994.electronicwerk`
  or `sudo snap remove electronicwerk`.

To remove your settings as well, delete the settings folder listed above.
