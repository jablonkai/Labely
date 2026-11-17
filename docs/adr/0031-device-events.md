# ADR-0031: Device events

- Status: proposed
- Date: 2026-11-17

## Context
The Android shell polls `job.get` every 300 ms while a label prints, and it reads
`device.status` again after each print (ADR-0027). A printer whose BLE link drops stays in the
hub's connection list, so the next `device.connect` returns the dead connection. ADR-0029 moved
the push design to Phase 5, so Android and iOS share one callback. It must not deadlock when a
shell callback calls back into the engine, and the CLI, MCP and studio must not change.

## Decision
- **Job observer in core.** `PrintQueue::set_observer` takes a `JobObserver`
  (`Fn(&JobInfo)`). The queue calls it when a job is queued, starts, prints a page, finishes or
  is cancelled before it starts. It runs while the queue is locked, so events arrive in order,
  and it must only hand the info on. Page progress comes from a hook on `JobControl`
  (`JobControl::with_progress_hook`), which fires when a driver reports a new page count.
  Frontends that set no observer behave as before.
- **`DeviceEvents` in `labely-ffi`.** It is a foreign trait with `job_changed(job_json)` (the
  `job.get` result) and `device_changed(event_json)`. The shell sets it with
  `LabelyEngine::set_events`. Device events are
  `{"address", "connected", "device"?, "status"?, "lost"?}`. They are sent after a successful
  `device.connect` (with `device`), `device.disconnect`, `device.status` (with `status`), and
  when a BLE link drops without `device.disconnect` (`lost: true`).
- **One dispatcher thread.** The sources (the queue observer, `execute`, the BLE notify sink)
  send events down a channel. A `labely-events` thread per engine calls the shell, so a
  callback may run commands. A panicking callback does not stop later events. The thread ends
  with the engine.
- **Lost links.** `BleNotifySink::disconnected` reports a lost link once, and not after the
  engine closed the link itself. The dispatcher then removes the connection from the hub, so
  the next `device.connect` opens a new link. If a print holds the connection, the removal is
  skipped. The print fails, and `device.disconnect` clears the connection.
- **Android.** `Engine` keeps the latest state of each job in a `StateFlow`. `job(id)` and
  `finished(id)` replace the `job.get` polling in the editor and in `printLabel`, and no event
  can be missed between `print.submit` and the first read. `AppState` clears the printer when
  its link drops and shows a message.
- **iOS.** The Swift bindings carry the same protocol. A Swift test checks job and device
  events on `sim:`. The app uses it from 5.3.
- **Not pushed.** Battery and paper changes are only sent after a `device.status` call. The
  printer reports them only when asked, so the engine adds no timer.

## Consequences
- No command schema changed, so `API_VERSION` stays 3.
- Every shell gets job progress and dropped links without polling.
