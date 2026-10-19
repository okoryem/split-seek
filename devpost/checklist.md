---
doc: checklist
status: approved
---

# Build Checklist

Build mode: learn

## Slices

- [x] **1. Jev scores one of your real frames**
  Becomes usable: A command you run in the terminal (`cargo run --example jev_probe -- <image> "<phrase>"`) that sends one frame from your footage to Jev and prints its 0–1 score and how long it took. The Tauri project exists and its window opens, though it's empty.
  Why now: Jev's image input is the one unverified assumption everything else depends on (`spec.md > Decisions and Open Issues`). If it fails, we fall back now, before any UI exists. Rust setup and the Tauri scaffold happen here because the probe needs them.
  PRD ref: `prd.md > Searching`, `prd.md > Open Questions`
  Spec ref: `spec.md > Stack`, `spec.md > File Structure`, `spec.md > External Services and Dependencies`, `spec.md > Components` (Jev Client)
  Build: Install Rust via rustup. Scaffold Tauri 2 with a plain HTML/JS frontend per the file structure, app name SplitSeek. Add `.env.example`, and extend `.gitignore` with `target/` and `node_modules/`. Write `jev.rs` (one request, Bearer key from `.env`, danger-zone header, `JEV_MODEL` with a `jev-preview` fallback) and `examples/jev_probe.rs`. Work out the real image format for `state` from the docs and by trial. Record what we found in the spec.
  Verify (mechanical): Pull one frame from a real clip with ffmpeg. Run the probe with a matching phrase and an obviously wrong one, and confirm a valid `noul` score for each, with the matching phrase scoring clearly higher. Confirm `npm run tauri dev` compiles and opens a window with no errors.
  Learner check: Run the probe on a frame you choose with two phrases, one right and one wrong, and say whether the scores make sense to you.
  Commit: `Scaffold Tauri app and verify Jev scores a frame`

- [x] **2. Search a folder of your clips from the terminal**
  Becomes usable: `cargo run --example search_cli -- <folder> "<phrase>"` prepares a folder (finds videos in subfolders, pulls LOG and boosted frames every 2 s into a temp folder) and prints ranked moments: clip name, `m:ss`, score, split at the 30% cutoff, plus frame count and elapsed time. A `--raw` flag scores the LOG frames instead of the boosted ones.
  Why now: This is the unique kernel, end to end, on real footage, before any UI. If matching quality or speed is poor, we learn it here, and the raw-vs-boosted comparison you agreed to in the spec happens here too. Later slices only put a window around it.
  PRD ref: `prd.md > Preparing Clips`, `prd.md > Searching`, `prd.md > Ranked Results`
  Spec ref: `spec.md > Components` (Folder Intake `scan.rs`, Frame Preparer `frames.rs`, Jev Client `jev.rs`, Search and Ranking `search.rs`), `spec.md > Data Model`, `spec.md > Important Failure Modes`
  Build: `scan.rs` (recursive; `.mp4`/`.mov`/`.m4v`; skips hidden and `.LRF`), `frames.rs` (one ffmpeg command with two outputs, 4 clips at a time, skip unreadable clips, temp folder deleted at the end), concurrency and retry in `jev.rs` (16 at once; back off on 429/5xx), and `search.rs` (rank, cutoff, timing log). Unit tests for scanning and timestamp math.
  Verify (mechanical): `cargo test` passes. Build a small fixture folder with ffmpeg test clips (nested, plus a `.LRF` and a hidden file) and confirm the right files and frame counts. Run `search_cli` on a real trip folder and confirm ranked output with timestamps and timing. Confirm the originals are unchanged (same size and modified time) and the temp folder is gone afterwards. Run the same 3–4 phrases with and without `--raw` and record the comparison in Revisions.
  Learner check: Point `search_cli` at a real trip folder, run three or four searches you'd actually make while editing, and say whether the top results are the right moments and whether the speed feels usable. Also say whether boosted or raw looks better from the comparison.
  Commit: `Add folder scan, frame prep, and ranked Jev search`

- [x] **3. Choose or drag in a folder, watch it prepare, and have it remembered**
  Becomes usable: The SplitSeek window in its dark Resolve-style look. "Choose folder" or dragging a folder in starts preparation with "Pulling frames from your clips… 12 / 48" progress, then the search box unlocks. Clips prepared before load from SplitSeek's frame cache without ffmpeg, so reopening a folder is fast. A folder with no videos shows "There are no videos in this folder." and stays on the picker. "Forget prepared frames" clears the cache.
  Why now: The engine is proven, so this connects it to the first half of the core journey. The look gets set here, while there's still room to change it.
  PRD ref: `prd.md > The Core Journey` (steps 1–3), `prd.md > Choosing a Folder`, `prd.md > Preparing Clips`, `prd.md > Look and Feel`, `prd.md > States and Boundaries`
  Spec ref: `spec.md > Look and Feel`, `spec.md > Components` (Folder Intake, Frame Preparer), `spec.md > Data Model` (`Prepared` app state)
  Build: The per-clip frame cache in `frames.rs` (key from path + size + modified time; `.partial` folder renamed on completion; clear-cache function), used by `search_cli` too. `index.html` with the three states, `styles.css` with the spec palette and typography, a `prepare_folder` command keeping `Prepared` in app state, `prepare-progress`/`prepare-done` events, a `forget_prepared_frames` command, the dialog plugin and drag-drop event, and capabilities.
  Verify (mechanical): `cargo test` and `cargo build` pass, including tests for cache keys and for an interrupted `.partial` folder being ignored. Prepare the fixture folder twice with `search_cli`: the second run reports every clip as cached and takes well under a second. Touch one clip and confirm only that clip is re-prepared. Run `npm run tauri dev` with no errors in the terminal or webview console; trigger preparation and confirm the progress events and frame count in the log. Confirm nothing is written to the clips' folder, and that "Forget prepared frames" empties the cache.
  Learner check: Open SplitSeek beside Resolve and drag in the folder of clips you copied to your Mac for the demo. Watch the progress, then quit, reopen, and drag it in again: the second time should be almost instant. Try an empty folder too. Say whether it looks and feels like it belongs next to Resolve.
  Commit: `Add folder picker, drag-drop, and preparation progress`

- [x] **4. Search in the window, see ranked moments, preview them, and jump to the clip**
  Becomes usable: The full core journey. Type a phrase, get ranked rows, one per stretch (LOG thumbnail of the best frame, file name, time range and best moment, confidence with an orange bar), use "Show more (N)" for near-misses, see the "came close" and "Nothing found" states, click a row to watch the original clip from its matched moment in a preview panel beside the list, and use the row's "⋯" menu to show the clip in Finder. Searching again needs no re-preparation.
  Why now: This completes the journey on top of a working engine and window, so it's mostly wiring and presentation. This is also where we check that previews don't slow the search.
  PRD ref: `prd.md > The Core Journey` (steps 4–7), `prd.md > Ranked Results`, `prd.md > Video Preview`, `prd.md > Reveal in Finder`, `prd.md > States and Boundaries`
  Spec ref: `spec.md > Components` (Search and Ranking, Results View, Video Preview, Reveal in Finder), `spec.md > Look and Feel`, `spec.md > Important Failure Modes` (missing key)
  Build: `search` and `reveal_in_finder` commands, base64 LOG previews on moments, the results rendering and Show more in `main.js`/`styles.css`, the empty and below-cutoff states, the "Jev API key missing — add it to .env" message, and "Choose another folder", a Search button, the two-pane layout with a `<video>` preview served by a `clip://` channel limited to the prepared folder's clips, and the "⋯" menu. Tune `CUTOFF` on real footage.
  Verify (mechanical): `cargo test`/`cargo build` pass. Run `npm run tauri dev` with no errors. Run searches on a real folder and compare against `search_cli` for the same phrase (same ranking). Compare the timing log with and without previews. Temporarily unset the key and confirm the message. Confirm `open -R` selects the file.
  Learner check: Run three or four real searches in the window, open "Show more" once, try a nonsense phrase, and click a result to land on the clip in Finder, then drag it into Resolve.
  Commit: `Add search results, video preview, and reveal in Finder`

- [x] **5. Ready for someone else to run**
  Becomes usable: A README that lets someone clone, add a key, and run SplitSeek, with every failure mode handled gracefully.
  Why now: Last, because it documents and hardens what actually got built. `6-ship` needs a runnable public repo.
  PRD ref: `prd.md > What We're Building`, `prd.md > Non-Goals`
  Spec ref: `spec.md > Where It Runs and How Someone Tries It`, `spec.md > Important Failure Modes`
  Build: A README covering what it is, requirements, setup, run, and how it works in brief. Check that the skipped-clip and failed-frame counts are logged. Remove leftover debug output.
  Verify (mechanical): From a clean clone in a temp directory, follow the README exactly (`npm install`, `.env`, `npm run tauri dev`) and confirm the app starts. Run a folder containing one corrupt "video" file and confirm it's skipped and counted. `cargo test` passes.
  Learner check: Read the README as if you were a stranger. Say whether anything is unclear or missing.
  Commit: `Add README and harden failure handling`

## Hands-on Checkpoints

- [x] Early usable behavior explored — after slice 2. The learner reviewed the real `flip/` results and gave feedback that changed the plan (one row per stretch; remember prepared clips) but did not run the terminal search themselves and chose to move on. Their first hands-on use of real footage is slice 3's learner check in the window.
- [x] Final kick-the-tires exploration and feedback completed — learner explored the full app; feedback: preview kept playing during a new search (fixed), cutoff question (kept at 60% on evidence).

## Final Review

- [x] Starting a new search stops and clears the preview — the clip kept playing and stuttered during the next search. Fixed in `main.js` (also cleared the remembered clip so re-clicking reloads it); verified in the browser; learner retried and confirmed.
- [x] Cutoff: learner asked about lowering to 50% after "speeding buggy" seemed to miss one. Checked: all three buggy clips appear above 60% (93–95%, and `C768` at 88%); only partial passes inside `C768` score 47–56%, and 50% would readmit known false hits (snowboarder 56%, Machu Picchu look-alikes 50–54%). Learner chose to keep 60%; no change.
- [x] Final review complete — feedback resolved and learner confirms ready to ship

## Code Tour and App Map

- [x] Learning activity complete — prior practice connected: the black-screen video diagnosis in slice 4
- [ ] Optional edit and transfer reflection addressed — offered/declined/already covered/not applicable as appropriate
- [x] `devpost/app-map.html` generated from finished code, checked, and shown, including a project-grounded practice to reuse

Activity and evidence: Focused alternative, already done during the build. The video preview passed browser checks but showed a black picture with audio in the real window; temporary diagnostics (learner clicked one result) showed the clip served as `text/html`; Tauri's `mime_type.rs` confirmed a lowercase-only `mp4` match; fixed with `clip_protocol.rs` and its tests; a second diagnostic run showed playback from 2.00 s and 64.00 s. Connected to the practice "instrument the real environment instead of guessing" (also the squashed-header bug).
Route and stops: Reference route only (not toured): `src/main.js` `runSearch` → `src-tauri/src/lib.rs` `async fn search(` → `src-tauri/src/search.rs` `fn group(`.
Edit outcome: Not applicable (focused alternative; learner-requested changes during final review were already made and committed).
Reflection: 
Activity mode: Prior practice connected, plus the app map.

## Revisions

- Jev images go in a top-level `assets` map referenced from `state`, and the yes/no descriptions are `criteria.true`/`criteria.false` — Jev's images docs showed the spec's `state`-only image and `yes`/`no` fields were wrong. Spec updated; verified on a Peru clip.
- Frames must stay 280–1568 px per side — Jev rejects, rather than resizes, out-of-range images. Slice 2 must keep frames inside these limits, including for unusually wide or tall footage.
- First probe on one Peru frame: boosted 84% vs. raw 68% on "young people at a mountain lake", both under 5% on wrong phrases. That's an early sign the boost helps; slice 2's comparison decides it.
- Frames are pulled from keyframes only, decoded on the Mac's video hardware, with a full-decode fallback when keyframes are sparse — full software decode of 4K 10-bit HEVC took 36 s for a 48 s clip vs. 1.1 s keyframe-only. Your Canon/DJI clips have a keyframe every 1.25 s, so timestamps stay within the agreed ±2 s.
- Frames scale so the short side is 288 px (16:9 → 512×288, vertical → 288×512) and are converted to full-range color — Jev rejects images under 280 px, and ffmpeg's JPEG encoder refused limited-range video from some sources.
- The scan follows linked files (not linked folders) — a linked clip was silently ignored in testing.
- `flip/` (47 clips, 43 GB, 55 min) took 19.5 min to prepare, entirely limited by the USB drive (~40 MB/s raw read). Seeking to each 2 s point instead read just as much data (15.1 s for a 625 MB clip), so it was not adopted. Searches took 14–15 s for 1,656 frames with 0 failures; the top hits for all four real phrases were correct when checked against the originals.
- Raw vs. boosted on `flip/`: no meaningful difference — top hits overlap, scores within a few points. Bring to the learner at the early checkpoint.
- A 30% cutoff let a wrong phrase ("a city skyline with skyscrapers") show 4–5 sunset frames at 34–57%; the cutoff likely needs to rise (tune in slice 4).
- Results are one row per stretch (back-to-back matching frames of a clip), not one per 2-second moment — at the early checkpoint the learner chose this after a 30-second match took 15 rows. PRD and spec updated; grouping lives in `search.rs`.
- Prepared frames are remembered per clip in `~/Library/Caches/com.splitseek.app/frames/` instead of a temp folder deleted on close — at the early checkpoint the learner asked for memory (all clips live on one slow Passport drive; first prep of `flip/` took ~20 min). This reverses the PRD's "no memory between sessions"; scope, PRD, spec, and slice 3 updated. Added to slice 3.
- The boost stays as is — raw vs. boosted showed no clear difference on `flip/`, and the learner didn't ask to change it.
- `prepare_folder` returns its summary directly instead of sending a separate `prepare-done` event, and the template's unused opener plugin was swapped for the dialog plugin — simpler, with the same behavior.
- The window's states were verified in a browser with a stand-in engine (Tauri windows can't be driven from the terminal); the real drag-and-drop was the learner's check. Their run cached 57 Canon clips in 22 MB, well under the 50–100 MB estimate.
- Cutoff raised from 30% to 60% — on the demo folder, correct top hits for seven real phrases scored 85–100% while wrong phrases peaked at 56% ("a snowboarder") and 48% ("an airplane cockpit").
- 64 Jev requests at once instead of 16 — searches on 637 frames dropped from ~5–7 s to ~2 s with no failures; 128 caused a failed frame and was slower, so 64 it is.
- "Show more" sends the best 30 near-miss stretches rather than all of them — a broad search can have 1,000+ stretches below the cutoff, and each carries a preview image.
- Renamed the confidence bar's CSS class from `.bar` to `.score-bar` — it shared a name with the top bar, which squashed the window header to 3 px. The learner caught it in the real window after the agent had dismissed the same 5 px offset in browser screenshots as a rendering artifact.
- Added a Search button (learner request: there was no visible cue to press Return) and video preview with a two-pane layout and a "⋯ → Show in Finder" menu (learner request during slice 4, moving playback out of "Deferred From the POC"). An AVFoundation check showed all three cameras' HEVC `hvc1` clips are playable on the Mac. PRD, spec, and slice 4 updated; default window widened to fit two sides.
- Video is served by SplitSeek's own `clip://` channel instead of Tauri's asset protocol — in the real window the preview was black with audio. Temporary diagnostics showed the clip labelled `text/html`: Tauri matches only a lowercase `mp4` extension, and its byte sniffing doesn't recognise Canon/DJI MP4s. The new channel labels by extension regardless of case, serves ranges, and allows only the prepared folder's clips (stricter than the folder-wide asset scope). Confirmed in the learner's window: playback started at the best moments (2.00 s, 64.00 s) in real time.
- A rejected Jev key (401/403) now reads "Jev rejected the API key — check JEV_API_KEY in .env" instead of raw server JSON, and `.env` is re-read with override so a fixed key works without restarting — found while testing failure handling in slice 5.
- The clean-clone check used a fresh copy of the tracked files plus the README (the README isn't committed until the learner's check), with no `.env`, build output, or `node_modules`: `npm install`, `cp .env.example .env`, and `npm run tauri dev` built and opened the app with no errors.
- The README describes SplitSeek as a standalone app beside any video editor, not Resolve specifically — the learner noted that naming DaVinci Resolve made it read as required or built for it.
