# ADR-0040: Phase 6 close-out

- Status: proposed
- Date: 2026-11-21

## Context
Phase 6 added print tasks, NIIMBOT variants, five drivers of other vendors, desktop packaging
and the release workflow (ADR-0035 to ADR-0039). None of it ran on hardware, and no release
workflow has run. The exit audit checked the five ADRs against `crates/protocol`,
`crates/printers`, `crates/transport`, the workflows, the Gradle and Xcode projects and the
docs. It found no open TODOs and no Decision that the code contradicts. It found four faults,
two gaps in the tests, and several places where the docs were behind.

## Decision
- **Hardware checks are not run.** They stay open, and Phase 6 is done when they pass:
  - The three iPhone checks of ADR-0034. No iPhone was available.
  - One label on the B21 Pro after the Phase 6 driver changes. The printer did not answer on
    BLE during this slice.
  - Nothing else can be checked: the B21 Pro is the only printer here, with one colour, no
    gray levels and no cutter (ADR-0036), and no printer of another vendor (ADR-0037).
- **TCP addresses are parsed, not guessed.** `tcp:fe80::1` became `fe80::1:9100`, which is
  another valid address, and a test asserted it. `host:99999` became `host:99999:9100` and
  failed as a name lookup. Now a bare IPv6 address gets brackets, `[v6]` works without a port,
  and a missing host or a port outside 1 to 65535 fails with a message that names the fault.
- **A repeated page index is not progress.** The old D11 task waits for PageIndex reports and
  restarted its stall timer on every report. A printer that repeats one index would never
  time out. Only a higher index restarts the timer now.
- **No wait after the last retry.** The B21 tasks slept 500 ms after the last failed attempt
  before they reported the failure.
- **A pre-release has its own Android version code.** `0.2.0-rc.1` and `0.2.0` had the same
  code, so Play would refuse the second. The code is now
  `(major * 1000000 + minor * 1000 + patch) * 100 + stage`. `stage` is the trailing number of
  the suffix (at most 98), and 99 for the release. `-Plabely.versionCode` still overrides it.
  This replaces the formula of ADR-0039; nothing was uploaded under the old one.
- **The studio job installs dist only when it uses it.** In a release the dist jobs build the
  CLI archives, and `release-studio.yml` still compiled cargo-dist on three systems.
- **Tests.**
  - The per-model test asserted "more than 40" models. It now asserts all 66.
  - A new test prints two pages with two copies on every NIIMBOT model, so the page counting
    of each task is checked past one page.
  - TCP address tests cover IPv6, bad ports and a missing host.
- **Docs.** `printers.md` points to `protocol-languages.md` for the other vendors.
  `architecture.md` lists `tcp:` and `sim:`. `protocol.md` says the gray check-line interval is
  unused. `releasing.md` says what an unsigned release looks like, that the tag must be the
  `Cargo.toml` version, and that the first tag should be a pre-release.
- **Left as it is, with reasons.**
  - *Unsigned releases stay green* (ADR-0039). The secrets are optional so that the workflows
    can be tried before the certificates exist. `releasing.md` now tells the releaser to read
    the notices.
  - *No check that the tag is the Cargo version.* dist plans nothing for a tag that matches no
    package version. UNCERTAIN until the first tag; this is one reason it should be an rc.
  - *Cancel between steps.* TSPL and ZPL send a whole job as one step, so a cancel waits for
    the send, at most 30 s per write on TCP. A cancel inside a page would leave half a label.
  - *Error-path tests of the new tasks* (a stalled D11, an L2B that never accepts). They
    need new simulator switches, which are better written against a real D11 or B21. The
    repeated-index fix has no test of its own for the same reason.
- **Deferred to Later.** The feature matrix has a Phase 6 exit status, and the roadmap lists
  the follow-ups under Later: the editor colour and the gray render path, the cut options, the
  mobile BLE link for Phomemo and Marklife, a model picker for `usb:` and `tcp:`, the USB
  printer class, the Marklife P50, C1 tube printing, speed and print mode. Each needs hardware
  that is not here.

## Consequences
- No schema or API version change.
- The roadmap marks slice 6.6 as audited, with the hardware checks open. The results are added
  to this ADR when they run.
- Signing, notarisation, the store listings and uploads, and a public repository stay open as
  ADR-0038 and ADR-0039 left them. They need accounts and certificates, not code.
