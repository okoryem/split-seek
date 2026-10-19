---
doc: checklist
status: approved
---

# Build Checklist

Build mode: [not chosen yet]

## Slices

- [ ] **1. Jev scores one of your real frames**
  Becomes usable: A command you run in the terminal (`cargo run --example jev_probe -- <image> "<phrase>"`) that sends one frame from your footage to Jev and prints its 0–1 score and how long it took. The Tauri project exists and its window opens, though it's empty.
  Why now: Jev's image input is the one unverified assumption everything else depends on (`spec.md > Decisions and Open Issues`). If it fails, we fall back now, before any UI exists. Rust setup and the Tauri scaffold happen here because the probe needs them.
  PRD ref: `prd.md > Searching`, `prd.md > Open Questions`
  Spec ref: `spec.md > Stack`, `spec.md > File Structure`, `spec.md > External Services and Dependencies`, `spec.md > Components` (Jev Client)
  Build: Install Rust via rustup. Scaffold Tauri 2 with a plain HTML/JS frontend per the file structure, app name SplitSeek. Add `.env.example`, and extend `.gitignore` with `target/` and `node_modules/`. Write `jev.rs` (one request, Bearer key from `.env`, danger-zone header, `JEV_MODEL` with a `jev-preview` fallback) and `examples/jev_probe.rs`. Work out the real image format for `state` from the docs and by trial. Record what we found in the spec.
  Verify (mechanical): Pull one frame from a real clip with ffmpeg. Run the probe with a matching phrase and an obviously wrong one, and confirm a valid `noul` score for each, with the matching phrase scoring clearly higher. Confirm `npm run tauri dev` compiles and opens a window with no errors.
  Learner check: Run the probe on a frame you choose with two phrases, one right and one wrong, and say whether the scores make sense to you.
  Commit: `Scaffold Tauri app and verify Jev scores a frame`

- [ ] **2. Search a folder of your clips from the terminal**
  Becomes usable: `cargo run --example search_cli -- <folder> "<phrase>"` prepares a folder (finds videos in subfolders, pulls LOG and boosted frames every 2 s into a temp folder) and prints ranked moments: clip name, `m:ss`, score, split at the 30% cutoff, plus frame count and elapsed time. A `--raw` flag scores the LOG frames instead of the boosted ones.
  Why now: This is the unique kernel, end to end, on real footage, before any UI. If matching quality or speed is poor, we learn it here, and the raw-vs-boosted comparison you agreed to in the spec happens here too. Later slices only put a window around it.
  PRD ref: `prd.md > Preparing Clips`, `prd.md > Searching`, `prd.md > Ranked Results`
  Spec ref: `spec.md > Components` (Folder Intake `scan.rs`, Frame Preparer `frames.rs`, Jev Client `jev.rs`, Search and Ranking `search.rs`), `spec.md > Data Model`, `spec.md > Important Failure Modes`
  Build: `scan.rs` (recursive; `.mp4`/`.mov`/`.m4v`; skips hidden and `.LRF`), `frames.rs` (one ffmpeg command with two outputs, 4 clips at a time, skip unreadable clips, temp folder deleted at the end), concurrency and retry in `jev.rs` (16 at once; back off on 429/5xx), and `search.rs` (rank, cutoff, timing log). Unit tests for scanning and timestamp math.
  Verify (mechanical): `cargo test` passes. Build a small fixture folder with ffmpeg test clips (nested, plus a `.LRF` and a hidden file) and confirm the right files and frame counts. Run `search_cli` on a real trip folder and confirm ranked output with timestamps and timing. Confirm the originals are unchanged (same size and modified time) and the temp folder is gone afterwards. Run the same 3–4 phrases with and without `--raw` and record the comparison in Revisions.
  Learner check: Point `search_cli` at a real trip folder, run three or four searches you'd actually make while editing, and say whether the top results are the right moments and whether the speed feels usable. Also say whether boosted or raw looks better from the comparison.
  Commit: `Add folder scan, frame prep, and ranked Jev search`

- [ ] **3. Choose or drag in a folder and watch it prepare**
  Becomes usable: The SplitSeek window in its dark Resolve-style look. "Choose folder" or dragging a folder in starts preparation with "Pulling frames from your clips… 12 / 48" progress, then the search box unlocks. A folder with no videos shows "There are no videos in this folder." and stays on the picker.
  Why now: The engine is proven, so this connects it to the first half of the core journey. The look gets set here, while there's still room to change it.
  PRD ref: `prd.md > The Core Journey` (steps 1–3), `prd.md > Choosing a Folder`, `prd.md > Preparing Clips`, `prd.md > Look and Feel`, `prd.md > States and Boundaries`
  Spec ref: `spec.md > Look and Feel`, `spec.md > Components` (Folder Intake, Frame Preparer), `spec.md > Data Model` (`Prepared` app state)
  Build: `index.html` with the three states, `styles.css` with the spec palette and typography, a `prepare_folder` command keeping `Prepared` in app state, `prepare-progress`/`prepare-done` events, the dialog plugin and drag-drop event, and capabilities. The temp folder is replaced when a new folder is chosen and deleted when the app closes.
  Verify (mechanical): `cargo test` and `cargo build` pass. Run `npm run tauri dev` with no errors in the terminal or webview console. Trigger preparation on the fixture folder and confirm the progress events and frame count in the log. Confirm the temp folder is replaced on a second pick and removed on quit.
  Learner check: Open SplitSeek beside Resolve, drag in a trip folder, watch the progress, then try an empty folder. Say whether it looks and feels like it belongs next to Resolve.
  Commit: `Add folder picker, drag-drop, and preparation progress`

- [ ] **4. Search in the window, see ranked moments, and jump to the clip**
  Becomes usable: The full core journey. Type a phrase, get ranked rows (LOG thumbnail, file name, `m:ss`, confidence with an orange bar), use "Show more (N)" for near-misses, see the "came close" and "Nothing found" states, and click a row to reveal the clip in Finder. Searching again needs no re-preparation.
  Why now: This completes the journey on top of a working engine and window, so it's mostly wiring and presentation. This is also where we check that previews don't slow the search.
  PRD ref: `prd.md > The Core Journey` (steps 4–7), `prd.md > Ranked Results`, `prd.md > Reveal in Finder`, `prd.md > States and Boundaries`
  Spec ref: `spec.md > Components` (Search and Ranking, Results View, Reveal in Finder), `spec.md > Important Failure Modes` (missing key)
  Build: `search` and `reveal_in_finder` commands, base64 LOG previews on moments, the results rendering and Show more in `main.js`/`styles.css`, the empty and below-cutoff states, the "Jev API key missing — add it to .env" message, and "Choose another folder". Tune `CUTOFF` on real footage.
  Verify (mechanical): `cargo test`/`cargo build` pass. Run `npm run tauri dev` with no errors. Run searches on a real folder and compare against `search_cli` for the same phrase (same ranking). Compare the timing log with and without previews. Temporarily unset the key and confirm the message. Confirm `open -R` selects the file.
  Learner check: Run three or four real searches in the window, open "Show more" once, try a nonsense phrase, and click a result to land on the clip in Finder, then drag it into Resolve.
  Commit: `Add search results, Show more, and reveal in Finder`

- [ ] **5. Ready for someone else to run**
  Becomes usable: A README that lets someone clone, add a key, and run SplitSeek, with every failure mode handled gracefully.
  Why now: Last, because it documents and hardens what actually got built. `6-ship` needs a runnable public repo.
  PRD ref: `prd.md > What We're Building`, `prd.md > Non-Goals`
  Spec ref: `spec.md > Where It Runs and How Someone Tries It`, `spec.md > Important Failure Modes`
  Build: A README covering what it is, requirements, setup, run, and how it works in brief. Check that the skipped-clip and failed-frame counts are logged. Remove leftover debug output.
  Verify (mechanical): From a clean clone in a temp directory, follow the README exactly (`npm install`, `.env`, `npm run tauri dev`) and confirm the app starts. Run a folder containing one corrupt "video" file and confirm it's skipped and counted. `cargo test` passes.
  Learner check: Read the README as if you were a stranger. Say whether anything is unclear or missing.
  Commit: `Add README and harden failure handling`

## Hands-on Checkpoints

- [ ] Early usable behavior explored — after slice 2: real searches on your footage from the terminal, plus raw vs. boosted. Match quality and speed here decide the cutoff, the boost, and whether anything needs to change before we build UI.
- [ ] Final kick-the-tires exploration and feedback completed

## Final Review

- [ ] Final review complete — feedback resolved and learner confirms ready to ship

## Code Tour and App Map

- [ ] Learning activity complete — guided route, focused alternative, prior practice connected, or brief recap
- [ ] Optional edit and transfer reflection addressed — offered/declined/already covered/not applicable as appropriate
- [ ] `devpost/app-map.html` generated from finished code, checked, and shown, including a project-grounded practice to reuse

Activity and evidence: 
Route and stops: 
Edit outcome: 
Reflection: 
Activity mode: 

## Revisions
