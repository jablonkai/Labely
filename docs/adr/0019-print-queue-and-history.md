# ADR-0019: Print queue and print history

- Status: proposed
- Date: 2026-11-15

## Context
Slice 3.5a of the feature matrix: the vendor app has a print-jobs panel (queued and finished
jobs, pause, resume, cancel) and a content history capped at 10 entries for non-members.
Until now `print.submit` blocked the caller until the printer finished, so the studio froze
while printing and a job could not be stopped. Labely keeps the history local and unlimited.
Multi-device printing (several printers at once) stays "later" in the matrix.

## Decision
- **Queue in core.** `labely_core::jobs::PrintQueue` belongs to the `Session`. `print.submit`
  still renders on the caller's thread (so render errors are reported at once), then submits
  the rasters. One worker thread, started on demand and stopped when the queue is empty,
  sends jobs to their printers in order. Jobs are `job-<n>`, with state `queued`, `printing`,
  `done`, `failed` or `cancelled`, and live progress. The list keeps the last 100 finished
  jobs; `job.clear` drops them.
- **Waiting.** `print.submit` waits for its job by default, so scripts, the CLI and existing
  agents see the same result as before (plus `job_id` and `state`). `wait: false` returns at
  once. A waited print on a paused queue fails with `bad_params` instead of hanging.
  `job.wait {id, timeout_ms}` blocks up to 10 minutes; in the studio agent calls run on the UI
  thread, so it waits at most 2 s per call there.
- **Pause and cancel.** Pausing applies to the queue: the running job finishes and no new
  job starts. Printers cannot pause mid-job reliably, so there is no per-job pause.
  `DeviceHub::print` and `Driver::print` take a `JobControl` (cancel flag in, progress out).
  Drivers stop at a page boundary: the NIIMBOT driver checks before the job, after
  `PrintStart` and after each `PageEnd`, and while it waits for pages, then sends
  `PrintEnd`. Pages already in the printer's buffer may still come out (UNCERTAIN on the
  B21 Pro until checked on hardware). Cancelling a queued job just drops it.
- **History.** A directory set by the frontend (`Session::set_history_dir`); the CLI, the
  headless MCP server and the studio use `<app-data>/history`. `history.json` lists every job
  that reached a printer (done, failed, or cancelled while printing); each entry has a
  `<id>.labely` snapshot of the document as printed. Entry ids are local timestamps
  (`YYYYMMDD-HHMMSS[-n]`). The index is rewritten through a temp file and a rename. A failed
  history write never fails the print. Like the template library, entries are addressed by
  id, so the history is not limited by the policy's file roots.
- **Commands.** `job.list`, `.get`, `.wait`, `.cancel`, `.pause`, `.resume`, `.clear`,
  `.history`, `.history_open` (opens the snapshot as a new unsaved document),
  `.history_delete` and `.history_clear`. History writes are not undoable and not
  journaled. `job.*` is not gated by "Allow agents to print": none of them starts a print.
- **FakePrinter.** `fake:<dir>?delay_ms=<n>` makes each page take `n` ms, so the queue,
  progress and cancel can be watched without hardware.
- **Studio.** The print dialog queues the job (`wait: false`) and opens the printers panel,
  which shows the queue with progress, cancel, pause/resume and clear. The status bar reports
  finished and failed jobs. Print → Print history… lists entries with open and delete, and
  clears the history.

## Consequences
- API version stays 3 (no release since it was bumped); the schema hash changed.
- `PrintOutput` gains `job_id` and `state`; existing fields keep their meaning.
- Not done in this slice: printing to several printers at once (the single worker serialises
  jobs across printers), and the label paper database, RFID auto-match, calibration and
  device settings (slice 3.5b).
