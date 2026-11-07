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

## The update check fails

- Check your internet connection.
- GitHub limits anonymous requests to 60 per hour and network. Try again later.
- Firewalls or proxies must allow `api.github.com`.

The [Updates](./updates) page has more details.

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
