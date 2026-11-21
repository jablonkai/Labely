# ADR-0039: Store builds and releases (Phase 6.5)

- Status: proposed
- Date: 2026-11-21

## Context
Slice 6.5 of ADR-0035 publishes what slice 6.4 packages. ADR-0038 left dist with `ci = []` and a
workflow that only built the desktop packages. The mobile shells had no release build: the
Android app had no signing config and a fixed version code, the iOS app had never been archived,
and both repeated the version of `Cargo.toml` by hand.

The development machine has no Developer ID certificate, no Play upload key and no App Store
Connect key. GitHub Actions is stopped for the account, so no workflow can run yet.

## Decision
- **A tag releases.** Pushing a tag such as `v0.2.0` runs `.github/workflows/release.yml`, which
  dist generates (`ci = "github"`). It builds the CLI archives and installers, creates the
  GitHub release and uploads every file. A pull request only runs `dist plan`.
- **Custom jobs for the rest.** dist calls three reusable workflows:
  - `release-studio.yml` builds the studio packages next to the CLI archives. It replaces
    `package.yml` and still runs alone on packaging changes.
  - `release-mobile.yml` builds the store builds.
  - `release-checksums.yml` runs after the release is published.
  Whatever a job uploads as an `artifacts-*` artefact goes into the GitHub release.
- **SHA256SUMS.** The checksum job downloads the files of the published release and uploads one
  `SHA256SUMS` over all of them. dist's own `sha256.sum` only knows the CLI files, and no dist
  job runs after both the installers and the custom jobs but before the release.
- **One version.** `Cargo.toml` holds the version. The Android build reads it. Its version code
  is `major * 1000000 + minor * 1000 + patch`, and `-Plabely.versionCode` overrides it. The
  release passes the version and the run number as build number to `xcodebuild`. The Xcode
  project still repeats the version, and an xtask test compares it with `Cargo.toml`.
- **Android.** The release build signs with an upload key from four `LABELY_KEY*` variables and
  is unsigned without them. The job builds an AAB for Play and an APK. Only a signed APK goes
  into the GitHub release.
- **iOS.** The job archives the app. With an App Store Connect key Xcode signs with
  cloud-managed certificates and exports an IPA after `ExportOptions.plist`. Without a key it
  builds an unsigned archive, which only shows that the app builds.
- **Stores by hand.** The AAB and the IPA stay workflow artefacts (`store-android`,
  `store-ios`). Uploading them to Play and to App Store Connect is a manual step. Play does not
  accept the first upload of an app through its API anyway, and the listings do not exist yet.
- **Secrets are optional.** Every signing step is skipped when its secrets are missing, so a
  release without certificates still publishes unsigned desktop packages. An unset secret is an
  empty variable, which cargo-packager takes as a value; the workflows only set the variables
  when the secrets are there, and the xtask ignores an empty `LABELY_SIGNING_IDENTITY`.
  [releasing.md](../releasing.md) lists the secrets.
- **dist 0.34.0 workaround.** dist 0.34.0 writes the custom jobs as
  `uses: $/.github/workflows/...`, which GitHub rejects; 0.33.1 wrote `./`. The three lines are
  fixed by hand, `allow-dirty = ["ci"]` stops dist from comparing the file, and an xtask test
  fails if the bug comes back.

## Consequences
- No workflow has run. `actionlint` accepts the four files and `dist plan` accepts the config,
  and that is all the proof there is. The first tag is the first test, so it should be a
  pre-release such as `v0.1.0-rc.1`.
- Built on the Mac: a release AAB and APK for arm64 with R8, signed with a throwaway key, and an
  unsigned iOS archive with the version and build number set from outside. The minified Android
  app has not been started on a device. No signed iOS archive or IPA has been built.
- Nothing has been uploaded to a store. The Play and App Store listings, the data safety form
  and the privacy details are open.
- With `allow-dirty`, dist no longer notices when `release.yml` is out of date. After a change
  to `dist-workspace.toml` someone has to run `dist generate` and repeat the fix.
- `SHA256SUMS` appears a moment after the release, not with it.
- The MSI is still not signed.
- The `repository` of `Cargo.toml` is now `github.com/jablonkai/Labely`, where the code is;
  the project does not move (this settles the open point of ADR-0038). The repository is
  private, so the shell and PowerShell installers, which download from it, fail until it is
  public.
- The iOS build number is the run number of the release workflow. A build uploaded by hand from
  Xcode must use a higher number than the last run.
