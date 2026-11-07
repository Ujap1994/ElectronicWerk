# Building from source

## Requirements

| Tool    | Version                                            |
| ------- | -------------------------------------------------- |
| Node.js | 22.12 or newer (CI uses 24)                        |
| npm     | 10 or newer                                        |
| Rust    | stable, installed with [rustup](https://rustup.rs) |
| Git     | any recent version                                 |

On Linux, the end-to-end tests need a display server (`xvfb-run` in CI).

## Run the app

```sh
git clone https://github.com/Ujap1994/ElectronicWerk.git
cd ElectronicWerk
npm install       # also installs the Git hooks
npm run dev       # builds the Rust engine and starts the app with hot reload
```

## Everyday commands

| Command                    | What it does                                          |
| -------------------------- | ----------------------------------------------------- |
| `npm run lint`             | All lint checks (must report zero warnings)           |
| `npm run format`           | Format TypeScript, CSS, Markdown and Rust             |
| `npm test`                 | Rust, tooling, unit and integration tests             |
| `npm run test:e2e`         | End-to-end tests (run `npm run build` first)          |
| `npm run build`            | Release build of the engine and the app               |
| `npm run icons`            | Regenerate every icon from `assets/icon/icon.svg`     |
| `npm run docs:dev`         | Documentation site with live reload                   |
| `npm run docs:screenshots` | Regenerate the documentation screenshots from the app |

## Packaging locally

```sh
npm run build:native -- --release            # native engine for this machine
npm run build -w @electronicwerk/desktop     # bundle the app
cd apps/desktop
npx electron-builder --mac --arm64 dir       # or --win / --linux, see electron-builder docs
```

For a macOS universal build, build the engine with
`npm run build:native -- --release --universal` first.

Check a packaged app with the built-in smoke test, which starts the app, waits
for the engine connection and exits with code 0:

```sh
release/mac-arm64/ElectronicWerk.app/Contents/MacOS/ElectronicWerk --smoke-test
```

## Documentation screenshots

Every screenshot in this documentation is generated from the real app by
`apps/desktop/e2e/docs/screenshots.spec.ts`, using synthetic demo data only.
After changing the user interface, run `npm run docs:screenshots` and commit
the updated images in `docs/public/screenshots/`.

See [CONTRIBUTING.md](https://github.com/Ujap1994/ElectronicWerk/blob/main/CONTRIBUTING.md)
for the rules every change must follow.
