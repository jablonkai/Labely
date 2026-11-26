---
name: render-golden
description: Update, review and bless Labely's render golden images (the pixel-exact 1-bit PNGs in crates/render/tests/golden/) after a change to rendering, fonts, barcodes, dithering, text layout or anything else that alters the print raster. Use this whenever `cargo test -p labely-render` fails with "pixels differ", "size differs" or "missing golden", whenever the user asks to bless, refresh, regenerate or update goldens or snapshots, or right after you change crates/render so the goldens are checked before committing. Triggers include "bless the goldens", "update the render goldens", "golden test fails", "pixels differ", and Hungarian "frissítsd a goldeneket", "blesseld a goldeneket", "elhasalt a golden teszt", "nézd át a golden képeket", "újra kell generálni a golden PNG-ket". Not for the core insta schema snapshot (that is the API_VERSION / schema-hash flow in AGENTS.md).
---

# Render goldens

Labely's renderer is deterministic and 1-bit, so its golden tests compare whole label rasters
pixel for pixel. That makes them very sensitive: an unintended one-pixel shift in a baseline is
a real print regression on paper. Blessing rewrites the reference, so a careless bless silently
accepts a regression. The job of this skill is to make sure only *intended* pixel changes land.

## What exists

- Goldens: `crates/render/tests/golden/*.png` (one per `golden("<name>", …)` call in
  `crates/render/tests/it/main.rs`; names end in the dpi, e.g. `text_align_203`, `codes_300`).
- Helper: `golden()` in `crates/render/tests/it/main.rs`. On a mismatch it panics with
  `<name>: N pixels differ; actual written to <path>` and writes `<name>.actual.png` into
  `CARGO_TARGET_TMPDIR` (under `target/tmp/`). A size change panics with `size differs`
  and writes no actual file.
- `cargo xtask bless` = `LABELY_BLESS=1 cargo test -p labely-render` (see `xtask/src/main.rs`).
  It rewrites **every** golden in that crate, and creates missing ones. It is the only golden
  set: there are no other `LABELY_BLESS` users and no other `tests/golden` directories.
  Unchanged renders produce byte-identical PNGs, so git only shows the ones that really moved.
- Not covered here: `crates/core/tests/it/snapshots/*.snap` (insta, schema; AGENTS.md
  "Schema changes") and the studio's egui_kittest tests.

## Workflow

### 1. Run the tests and collect the failures

```sh
cargo test -p labely-render 2>&1 | grep -E "pixels differ|size differs|missing golden|test result"
```

Filter the output; the full cargo log is noise. Write down each failing name, its pixel
count, and the actual-PNG path.

If nothing fails, stop: there is nothing to bless. Do not bless "just in case" — with no
failures it is a no-op at best and hides nothing worth hiding.

### 2. Decide, per golden, whether the change is intended

Compare each failure against what the code change was meant to do. Look at both images with
the Read tool, which displays PNGs:

- the committed golden: `crates/render/tests/golden/<name>.png`
- the new render: the `<name>.actual.png` path from the panic message

Ask: does the difference match the change? A text-layout fix should move text in the text
goldens, not in `codes_300` or `image_dither_203`. Small pixel counts can still matter (a
clipped glyph edge, a barcode module one dot narrower makes a scan fail). A golden that
changed but is unrelated to your change is a bug to fix, not to bless.

If a change is unintended, fix the code and go back to step 1. Report it to the user rather
than blessing around it.

### 3. Bless

```sh
cargo xtask bless 2>&1 | grep -E "test result|error"
```

### 4. Review the diff before committing

```sh
git status --short crates/render/tests/golden/
git diff --stat crates/render/tests/golden/
```

- Only the goldens you judged intended in step 2 should be modified. If another one changed,
  something is non-deterministic or you missed a failure; investigate before going on.
- A new `??` file means a new `golden(...)` call; view it with Read and confirm it shows what
  the test describes.
- View every modified PNG once more with Read (git cannot diff binaries, and the reviewer of
  the PR will look at the images, so you should first).

Then rerun without bless to prove the goldens pass:

```sh
cargo test -p labely-render 2>&1 | grep "test result"
```

### 5. Commit with the reason

Commit the goldens together with the code change that caused them, never as a separate
unexplained "update goldens" commit. Say in the message body which goldens changed and why,
so a reviewer can judge the images against the claim. For example:

```
fix(render): centre text on the cap height, not the em box

Goldens text_align_203, text_align_300 and text_styles_203 move every line
up by 1-2 dots; that is the intended fix. No other golden changed.
```

Report to the user: which goldens changed, the pixel counts from step 1, and that you viewed
each image.
