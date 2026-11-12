# Releases

ElectronicWerk follows [Semantic Versioning](https://semver.org/). Until 1.0,
every finished and tested feature leads to a new `0.x` release, so every step is
traceable.

## What a release contains

| System  | Packages                                                                                 |
| ------- | ---------------------------------------------------------------------------------------- |
| Windows | NSIS installer and portable `.exe`, each for x64 and arm64                               |
| macOS   | `.dmg` and `.zip` for universal, Apple Silicon (arm64) and Intel (x64)                   |
| Linux   | AppImage, `.deb`, `.rpm`, `.pacman`, `.tar.gz`, Snap and Flatpak, each for x64 and arm64 |
| CLI     | `ewerk` archives for every system and architecture                                       |

Plus `SHA256SUMS.txt` and release notes taken from the changelog.

## How a release is made

1. Move the `Unreleased` entries in `CHANGELOG.md` into a new section
   `## [x.y.z] - YYYY-MM-DD`.
2. Set the version everywhere:

   ```sh
   node scripts/version.mjs set x.y.z
   npm install && cargo check   # update the lock files
   ```

3. Update the roadmap, commit (`chore(release): x.y.z`) and push.
4. Tag and push the tag:

   ```sh
   git tag vx.y.z
   git push origin vx.y.z
   ```

The [release workflow](https://github.com/Ujap1994/ElectronicWerk/blob/main/.github/workflows/release.yml)
then:

1. checks that the tag matches the version in every manifest,
2. builds every package on native runners (Windows x64 and arm64, macOS, Linux
   x64 and arm64),
3. starts every packaged app once in smoke-test mode,
4. packs the CLI,
5. publishes the GitHub release with the notes, the download guide and the
   checksums.

The in-app [update notice](../guide/updates) picks up the new release
automatically.

## When the Snap Store is unreachable

Building a snap needs `snapcraft` from the Snap Store. Now and then the store
cannot be reached from GitHub's runners for hours. The release workflow tries
for about ten minutes; if the store stays away, it builds the other Linux
packages of that architecture without the snap, publishes the release anyway
and shows a warning in the run summary.

To add the missing snap later, run **Actions → Release snap → Run workflow**
with the tag (for example `v0.7.1`) and the architecture (`x64` or `arm64`).
The workflow:

1. checks that the release exists and has no snap for that architecture yet,
2. builds the engine, the app and the snap from the tagged source,
3. starts the app once in smoke-test mode,
4. uploads the snap to the release and replaces `SHA256SUMS.txt` with a list
   that includes it.

If the Snap Store is still unreachable, the workflow fails without changing
the release; run it again later.

## Test builds

Run the workflow manually (**Actions → Release → Run workflow**) to build and
smoke-test every package without publishing. The files are kept as workflow
artifacts for 14 days.
