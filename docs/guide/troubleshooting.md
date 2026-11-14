# Troubleshooting

## macOS: "ElectronicWerk cannot be opened" or "is damaged"

macOS blocks apps that are not notarized by Apple. Follow the steps in
[Installation → macOS](./installation#macos): open **System Settings → Privacy
& Security** and click **Open Anyway**, or run

```sh
xattr -dr com.apple.quarantine /Applications/ElectronicWerk.app
```

## Windows: "Windows protected your PC"

Click **More info**, then **Run anyway**. This message appears because the
installer is not signed with a paid certificate.

## Linux: the AppImage does not start

On Ubuntu 24.04 and newer, the system blocks the sandbox that the AppImage
needs. Install the `.deb` package instead (see
[Installation → Linux](./installation#linux)).

## The status bar shows "Engine stopped"

The engine (the part of ElectronicWerk that does the design work) could not
start, or it stopped several times in a row.

1. Quit ElectronicWerk and start it again.
2. If the message stays, reinstall ElectronicWerk with the package for your
   system and processor (an x64 package on an arm64 system, or the other way
   round, can cause this).
3. If it still does not work, [report an issue](https://github.com/Ujap1994/ElectronicWerk/issues)
   and include the full text of the message.

## "The engine stopped unexpectedly and was restarted"

ElectronicWerk restarted its engine automatically after a crash. You can keep
working; click **×** to dismiss the notice. If you can repeat what caused it,
please report it so it can be fixed.

## The 2D view stays empty or is slow

The [2D view](./viewer) is drawn by the graphics card with WebGL 2.

- **"The 2D view needs WebGL 2"**: the graphics driver does not offer WebGL 2.
  Update the driver (on Linux, install the Mesa drivers of your distribution).
  Without any usable graphics card, for example in a virtual machine or over
  a remote desktop, ElectronicWerk uses Chromium's software renderer, which
  works but is much slower on large boards.
- **Slow on a laptop with two graphics chips**: ElectronicWerk asks for the
  fast graphics chip. If the operating system still uses the slower one, set
  the graphics preference for ElectronicWerk in the system settings
  (Windows: _Settings → System → Display → Graphics_).
- To see which renderer is used, open _View → Toggle Developer Tools_ and
  enter `document.createElement('canvas').getContext('webgl2')` in the
  console.

## The 3D view misses parts or models

- **“… not found” in the status line**: the 3D models of these parts are
  neither in the project folder, in a KiCad installation nor in the base
  library. The list under **… not found** names the paths. Install KiCad,
  set up the [base library](./library), or copy the files into the project
  folder (see [Parts and their 3D models](./view-3d#parts-and-their-3d-models)).
- **A model takes long the first time**: STEP files are converted into
  meshes once; large connectors or modules can take several seconds. They are
  cached afterwards.
- **A part looks turned or offset**: check its model's offset and rotation in
  the [footprint editor](./footprint-editor#3d-models); the preview there
  shows the model on the footprint.
- **GLB or VRML export lists “has no mesh yet”**: show the board once in the
  3D view, then export again (see [Models for GLB and VRML](./export-3d#models-for-glb-and-vrml)).

## The update check fails

- Check your internet connection.
- GitHub limits anonymous requests to 60 per hour and network. Try again later.
- Firewalls or proxies must allow `api.github.com`.

The [Updates](./updates) page has more details.

## GitHub: sign-in, push or pull fail

The [GitHub page](./github#troubleshooting) explains every message of the
source control view. Firewalls or proxies must allow `github.com` and
`api.github.com`.

## Resetting all settings

Quit ElectronicWerk and delete `settings.json` from the settings folder
(see [Settings → Where the settings are stored](./settings#where-the-settings-are-stored)).

## Reporting a problem

Use **Help → Report an Issue** in the app or open an issue on
[GitHub](https://github.com/Ujap1994/ElectronicWerk/issues). Please include:

- the ElectronicWerk version (status bar, bottom right),
- your operating system and processor type,
- what you did, what you expected and what happened,
- screenshots if they help. Remove personal data from them first.
