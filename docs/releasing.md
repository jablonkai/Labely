# Releasing

A tag releases Labely (ADR-0039). The version is the `version` of `[workspace.package]` in
`Cargo.toml`; the desktop packages, the Android app and the release build of the iOS app take
it from there.

## Steps

1. Set the version in `Cargo.toml` and `MARKETING_VERSION` in the Xcode project (two places).
   `cargo xtask ci` fails when they differ.
2. Merge that to `main`.
3. Tag the commit and push the tag: `git tag v0.2.0 && git push origin v0.2.0`. A version with
   a suffix, such as `v0.2.0-rc.1`, becomes a pre-release.
4. The Release workflow builds everything and publishes the GitHub release:
   - the `labely` CLI archives for five targets with the shell and PowerShell installers,
   - the studio as dmg, MSI, AppImage and deb,
   - the Android APK, when the upload key is set,
   - `SHA256SUMS` over every file.
5. Download the `store-android` and `store-ios` artefacts of the run and upload the AAB to the
   Play Console and the IPA to App Store Connect (Transporter).

Check a download with `sha256sum --check --ignore-missing SHA256SUMS`.

## Secrets

Every secret is optional. Without it the step is skipped and the package is not signed.

| Secret | Used for |
|---|---|
| `MACOS_CERTIFICATE` | Developer ID Application certificate, a base64 `.p12` |
| `MACOS_CERTIFICATE_PASSWORD` | the password of that `.p12` |
| `MACOS_SIGNING_IDENTITY` | the identity, `Developer ID Application: Name (TEAMID)` |
| `APPSTORE_API_KEY` | App Store Connect API key, the text of the `.p8`; notarises the studio and signs the iOS app |
| `APPSTORE_API_KEY_ID` | the key id |
| `APPSTORE_API_ISSUER_ID` | the issuer id |
| `APPLE_TEAM_ID` | the team of the iOS app |
| `ANDROID_KEYSTORE` | Play upload keystore, base64 |
| `ANDROID_KEYSTORE_PASSWORD` | the keystore password |
| `ANDROID_KEY_ALIAS` | the key alias |
| `ANDROID_KEY_PASSWORD` | the key password |

The App Store Connect key needs the Admin role, because Xcode creates the distribution
certificate and the profile with it.

## Changing the release workflow

`.github/workflows/release.yml` is generated. Change `dist-workspace.toml`, run `dist generate`,
then change the three `uses: $/.github/workflows/` lines to `uses: ./.github/workflows/`
(a dist 0.34.0 bug; `cargo test -p xtask` checks it).

## Local release builds

```sh
cargo xtask package                                    # studio packages
dist build --artifacts=local --target <triple>         # CLI archive
(cd mobile/android && ./gradlew :app:bundleRelease :app:assembleRelease)   # AAB and APK
(cd mobile/ios/Labely && xcodebuild archive -quiet -project Labely.xcodeproj -scheme Labely \
  -destination 'generic/platform=iOS' -archivePath /tmp/Labely.xcarchive CODE_SIGNING_ALLOWED=NO)
```

The Android build signs when `LABELY_KEYSTORE_FILE`, `LABELY_KEYSTORE_PASSWORD`,
`LABELY_KEY_ALIAS` and `LABELY_KEY_PASSWORD` are set.
