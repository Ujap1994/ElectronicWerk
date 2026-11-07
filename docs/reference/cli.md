# Command-line tool (`ewerk`)

`ewerk` is ElectronicWerk's command-line interface. It contains the same engine
as the desktop app, so everything the engine can do is available in a
terminal, in scripts, in CI pipelines and to AI agents. The command set grows
with every milestone; with the project milestone (v0.2) it will create and
edit projects.

## Installation

Download the archive for your system from the
[latest release](https://github.com/Ujap1994/ElectronicWerk/releases/latest):

| System              | Archive                                                     |
| ------------------- | ----------------------------------------------------------- |
| Windows x64 / arm64 | `ewerk-<version>-win-x64.zip` / `…-win-arm64.zip`           |
| macOS Apple Silicon | `ewerk-<version>-mac-arm64.tar.gz`                          |
| macOS Intel         | `ewerk-<version>-mac-x64.tar.gz`                            |
| Linux x64 / arm64   | `ewerk-<version>-linux-x64.tar.gz` / `…-linux-arm64.tar.gz` |

Unpack it and move `ewerk` (`ewerk.exe` on Windows) to a folder on your `PATH`,
for example:

```sh
tar -xzf ewerk-0.1.0-linux-x64.tar.gz
sudo mv ewerk-0.1.0-linux-x64/ewerk /usr/local/bin/
ewerk --version
```

::: tip macOS
macOS quarantines downloaded programs. Run
`xattr -d com.apple.quarantine ewerk` once after unpacking.
:::

## Global options

| Option            | Effect                                                        |
| ----------------- | ------------------------------------------------------------- |
| `-h`, `--help`    | Show help (also per command, for example `ewerk call --help`) |
| `-V`, `--version` | Print the version                                             |

## Commands

### `ewerk info`

Shows information about the engine build.

```console
$ ewerk info
ElectronicWerk Engine 0.1.0
Target:  linux-x86_64
Threads: 16
Build:   release
```

With `--json`, the same information is printed as JSON for scripts:

```console
$ ewerk info --json
{
  "name": "ElectronicWerk Engine",
  "version": "0.1.0",
  "os": "linux",
  "arch": "x86_64",
  "threads": 16,
  "debug": false
}
```

### `ewerk methods`

Lists every engine method, one per line:

```console
$ ewerk methods
engine.info
engine.methods
engine.ping
```

### `ewerk call <method> [params]`

Calls an engine method directly and prints the JSON reply. `params` is a JSON
value and defaults to `null`. The [Engine API](./engine-api) describes all
methods.

```console
$ ewerk call engine.ping '{"hello": "world"}'
{"result":{"echo":{"hello":"world"},"pong":true}}

$ ewerk call engine.unknown
{"error":{"code":"methodNotFound","message":"unknown method `engine.unknown`"}}
```

::: tip Quoting on Windows
In PowerShell, wrap JSON in single quotes and escape inner double quotes:
`ewerk call engine.ping '{\"hello\": \"world\"}'`.
:::

## Exit codes

| Code | Meaning                                                             |
| ---- | ------------------------------------------------------------------- |
| `0`  | Success                                                             |
| `1`  | The engine reported an error, or the parameters were not valid JSON |
| `2`  | Wrong command-line usage (unknown command or option)                |

## Using `ewerk` in scripts

The JSON output works well with tools such as `jq`:

```sh
threads=$(ewerk info --json | jq .threads)
echo "The engine may use $threads threads."

if ewerk call engine.ping > /dev/null; then
  echo "Engine OK"
fi
```
