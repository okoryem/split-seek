---
doc: spec
status: approved
---

# SplitSeek — Technical Spec

## How This Works, In Plain Language
SplitSeek is a **Tauri** desktop app. Tauri is a framework (not a language) for building desktop apps in two halves:

- **The window (frontend):** a plain HTML/CSS/JavaScript page that macOS draws in its built-in web view. It shows the folder picker, progress, search box, and ranked results in a dark, Resolve-style look.
- **The engine (backend):** Rust code running on your Mac. It does everything a web page isn't allowed to do: list the videos in your folder, run **ffmpeg** (the command-line video tool) to pull frames, call **Jev** with your API key, and open Finder.

When you pick a folder, the engine has ffmpeg grab one frame every 2 seconds from every clip. It saves two small JPEGs per moment: the original **LOG** frame (for the preview) and a **boosted** copy with more contrast and color (for Jev). The JPEGs go into SplitSeek's cache folder on your Mac, one subfolder per clip, so a clip prepared once is never prepared again unless it changes. Your original clips are only ever read.

When you search, the engine asks Jev one yes/no question per boosted frame: "Does this frame show: *street market at night*?" Jev answers with a number from 0 to 1. The engine sends many of these questions at once, joins back-to-back matching frames of the same clip into one stretch, sorts the stretches by their best score, splits them at the cutoff (60%, tuned on real footage), and sends the list back to the window. Clicking a row plays the original clip in a preview panel, starting at the matched moment; a row's "⋯" menu tells the engine to reveal that clip in Finder.

Why this shape: there's no server, database, or account. It's one app on your laptop, and the only outside service is Jev, which is the point of the project. The prepared frames last only while the app is open, which is exactly what the PRD asks for.

## The Core Journey Through the System
PRD ref: `prd.md > The Core Journey`.

1. **Open the app** → the window shows the folder picker.
2. **Choose or drag in a folder** → the window sends the folder path to the engine (`prepare_folder`).
3. **Preparation** → the engine finds the video files, runs ffmpeg on several clips at a time, and sends a progress update after each clip. The window shows "Pulling frames from your clips… 12 / 48". When it finishes, the search box unlocks.
4. **Search** → the window sends the phrase (`search`). The engine sends one Jev yes/no question per boosted frame, many in parallel, and collects the scores.
5. **Ranked results** → the engine groups frames into stretches, sorts by score, splits the stretches into above-cutoff and below-cutoff lists, and attaches the LOG preview for each row. The window draws the rows, plus "Show more" if any moments fell below the cutoff.
6. **Click a row** → the preview panel plays the original clip from its best moment, served by the engine's `clip://` channel (only clips in the prepared folder). **"⋯" → Show in Finder** sends the clip path (`reveal_in_finder`), and Finder opens with that file selected.
7. **Search again** → this reuses the frames already prepared in memory and on disk, so there's no ffmpeg wait.

```
Window (HTML/JS)                 Engine (Rust)                       Outside
─────────────────                ─────────────                       ───────
pick/drag folder ──prepare──▶ scan videos ──▶ ffmpeg ──▶ temp JPEGs
progress bar     ◀──events───  per-clip progress
type phrase      ──search───▶ for each boosted frame ──────────────▶ Jev /v1/systemone
ranked rows      ◀──results──  sort + cutoff ◀── score 0–1 ◀──────── (yes/no answer)
⋯ Show in Finder ─reveal─▶ open -R <clip>  ──────────────────────▶ Finder
```

## Stack
- **Tauri 2** (Rust backend plus web frontend). Learner choice: they wanted to try Rust and use the coding agent to work in an unfamiliar language. Tradeoff accepted: Rust's learning curve and compile errors. Docs: https://v2.tauri.app/start/
- **Rust (stable, installed via rustup)**. Not installed yet; setting it up is an early build step. Docs: https://www.rust-lang.org/tools/install
- **Plain HTML/CSS/JavaScript frontend, no UI framework.** Agent recommendation, accepted by the learner at review: the window has one screen in three states, so React or similar would add setup without helping the proof.
- **Rust crates:** `tokio` (run things in parallel), `reqwest` (HTTP calls to Jev), `serde`/`serde_json` (JSON), `base64` (encoding frames), `dotenvy` (reading the API key from `.env`), `futures` (limiting how many Jev calls run at once), `tempfile` (temporary folders in tests).
- **Tauri plugins:** `tauri-plugin-dialog` for the folder picker (https://v2.tauri.app/plugin/dialog/). Drag-and-drop uses Tauri's built-in window drag-drop event. Reveal in Finder runs macOS's `open -R <path>`. Video preview uses SplitSeek's own `clip://` URI scheme (`clip_protocol.rs`), not Tauri's asset protocol.
- **ffmpeg / ffprobe 9.0.1**, already installed via Homebrew. Called as a command-line program. Docs: https://ffmpeg.org/ffmpeg-filters.html
- **Node v26 / npm**, already installed. Used only by the Tauri CLI (`npm run tauri dev`).

Versions were checked on this machine on 2026-10-19. Exact Tauri crate and plugin versions are pinned when the project is scaffolded.

## Where It Runs and How Someone Tries It
- **Runtime:** a local macOS desktop app (Apple Silicon, macOS 15). No hosting or deployment.
- **Requirements:** Rust (rustup), Node/npm, ffmpeg on the PATH, and a Jev API key with image ("danger zone") access.
- **Setup:** copy `.env.example` to `.env` and set `JEV_API_KEY=...`. `.env` is gitignored.
- **Start:** `npm install`, then `npm run tauri dev`. The SplitSeek window opens.
- **Demo recording:** open SplitSeek beside Resolve, drag in one trip folder (about 20–50 clips with lookalikes), wait for preparation, then run three or four different searches and click one result to show Finder. Record with macOS screen recording (Cmd+Shift+5).
- **Submission:** a short demo video plus the public GitHub repository. `6-ship` handles both.

## Look and Feel
From `prd.md > Look and Feel`:
- **Palette:** Resolve-like charcoal greys. Window background about `#1e1e1e`, panels `#282828`, borders `#3a3a3a`, primary text `#d6d6d6`, muted text `#8a8a8a`, and a single orange accent (`#e8833a`) for focus, progress, and confidence bars.
- **Typography:** the system UI font (`-apple-system`, SF Pro) at 12–13px, with tabular numbers for timestamps and percentages, like an editor's inspector panel.
- **Density:** compact rows (thumbnail about 128px wide). Default window about 980×680 so the list and preview sit side by side; below about 760px wide the preview stacks above the list. Minimum window width about 420px.
- **Copy tone:** short, plain, and friendly ("Pulling frames from your clips…", "There are no videos in this folder.").
- **Previews:** the original flat LOG frame, never the boosted one.

## Components

### Folder Intake (frontend: `src/main.js`, backend: `scan.rs`)
- Shows the picker state with a "Choose folder" button (dialog plugin) and a drop zone (Tauri drag-drop event). Either one calls `prepare_folder(path)`.
- `scan.rs` walks the folder **including subfolders**, because SD cards nest clips under `DCIM/...`. It keeps `.mp4`, `.mov`, and `.m4v` (any case) and skips hidden files and DJI `.LRF` proxy files.
- If there are no videos, it returns an error that the window shows as "There are no videos in this folder." and stays on the picker.
- PRD ref: `prd.md > Choosing a Folder`, `prd.md > States and Boundaries`.

### Frame Preparer (`frames.rs`)
- **Frame cache:** `~/Library/Caches/com.splitseek.app/frames/<key>/`, one folder per clip. The key is a hash of the clip's full path, size, and modified time, so a renamed, edited, or replaced clip gets a fresh key. A clip whose folder exists is loaded without running ffmpeg. New clips are written to `<key>.partial/` and renamed when complete, so an interrupted preparation never leaves half a clip cached.
- For each clip, runs one ffmpeg command with two outputs, at most 4 clips at a time:
  - LOG preview: `fps=1/2,scale=512:-2` → `<clip-id>/log_%05d.jpg`
  - Boosted for Jev: `fps=1/2,scale=512:-2,eq=contrast=1.35:saturation=1.6:gamma=1.1` → `<clip-id>/boost_%05d.jpg`
  - Frame *n* maps to timestamp `(n-1) × 2` seconds.
- The boost values are a starting point, tuned in the raw-vs-boosted check (see Decisions).
- Sends a `prepare-progress` event `{done, total, clip}` after each clip, then `prepare-done` with the frame count.
- A clip ffmpeg can't read is skipped and counted, and preparation continues.
- A "Forget prepared frames" control deletes the whole cache folder. Nothing is ever written beside the original clips.
- PRD ref: `prd.md > Preparing Clips`.

### Jev Client (`jev.rs`)
- Sends one request per frame to `POST https://api.typesafe.ai/v1/systemone` (see External Services).
- Runs up to 64 requests at once (measured: 16 → ~6 s, 64 → ~2 s for 637 frames; 128 caused a failed frame and was slower). On a 429 or 5xx response it waits and retries up to 3 times. After that, the frame scores as "unknown" and is left out of the results.
- Reads `JEV_API_KEY` from `.env`. The key never reaches the frontend.
- The model name comes from an env var: `JEV_MODEL=jev-latest`, falling back to `jev-preview`.
- PRD ref: `prd.md > Searching`.

### Search and Ranking (`search.rs`)
- `search(phrase)` → scores every boosted frame → groups back-to-back frames of the same clip on the same side of the cutoff into one stretch (a `Moment`) → returns `{ above: Moment[], below: Moment[], elapsed_ms }`, sorted by each stretch's best score from highest to lowest.
- `CUTOFF = 0.60`, tuned on the learner's demo folder: correct top hits scored 85–100%, wrong phrases at most 56% (`prd.md > Product Decisions`).
- At most 30 near-miss stretches are sent for "Show more", each with a preview.
- Each moment includes its LOG preview as a base64 data URL, so the window doesn't need file access.
- Every search logs the frame count and elapsed time to the terminal. This is the measurement that decides whether speed work is needed.
- PRD ref: `prd.md > Searching`, `prd.md > Ranked Results`.

### Results View (frontend: `src/main.js`, `src/styles.css`)
- Each row shows the thumbnail of the best frame, file name, time range and best moment (`0:04–0:26 · best 0:12`), and confidence (percent plus a thin orange bar). There's one row per stretch.
- If any stretches are below the cutoff, a "Show more (N)" button reveals the best of them (up to 30) underneath.
- If `above` is empty but `below` isn't, the view shows "We couldn't find anything above 60%, but here are some that came close." with the `below` list expanded.
- If both are empty, it shows "Nothing found for '<phrase>'."
- A "Choose another folder" link returns to the picker.
- PRD ref: `prd.md > Ranked Results`, `prd.md > States and Boundaries`.

### Video Preview (frontend: `src/main.js`; permission: `lib.rs`)
- Clicking a row selects it and loads the original clip into a `<video>` element from `clip://localhost/<encoded path>`, seeking to the row's best moment and playing (muted by default so it can start on its own).
- `clip_protocol.rs` serves only clips in the prepared folder (403 otherwise), labelled `video/mp4` or `video/quicktime` regardless of extension case, in byte ranges of at most 4 MB. Nothing else on disk is readable by the window.
- PRD ref: `prd.md > Video Preview`.

### Reveal in Finder (`lib.rs` command)
- `reveal_in_finder(path)` runs `open -R <path>`, from each row's "⋯" menu.
- PRD ref: `prd.md > Reveal in Finder`.

## Data Model
All of this lives in the engine's memory as Tauri app state:

```rust
struct Frame  { clip_path: PathBuf, clip_name: String, t_sec: u32,
                log_jpg: PathBuf, boost_jpg: PathBuf }
struct Prepared { folder: PathBuf, frames: Vec<Frame>, clip_count, cached_count, skipped }
struct Moment { clip_path, clip_name, start_sec, end_sec, best_sec,   // one stretch
                score: f32, preview_data_url: String }      // best frame's
```

| Data | Where it lives | How it updates | When you leave and come back |
|---|---|---|---|
| Chosen folder plus frame list | Engine memory (`Prepared`) | Replaced when you pick a new folder | Rebuilt quickly from the frame cache |
| Frame JPEGs | `~/Library/Caches/com.splitseek.app/frames/<clip key>/` | Written once per clip during preparation | Persist until "Forget prepared frames" |
| Search results | Window memory | Replaced on each search | Gone |
| Jev API key | `.env` (gitignored) | Edited by hand | Persists |

Nothing is written to the original clips or to the clips' folder.

## File Structure
```
buildWithAi/                     # project root (repo)
├── src/                         # Frontend: the window
│   ├── index.html               # picker / preparing / search states in one page
│   ├── styles.css               # dark Resolve-style theme
│   └── main.js                  # calls engine commands, renders rows, Show more
├── src-tauri/                   # Engine (Rust)
│   ├── Cargo.toml               # Rust dependencies
│   ├── tauri.conf.json          # app name SplitSeek, window size, permissions
│   ├── capabilities/default.json# allows dialog + drag-drop
│   ├── examples/jev_probe.rs    # build step 1: send one frame to Jev, print answer
│   └── src/
│       ├── main.rs              # entry point
│       ├── lib.rs               # registers commands: prepare_folder, search, reveal_in_finder
│       ├── scan.rs              # find video files in folder
│       ├── frames.rs            # run ffmpeg, temp folder, progress events
│       ├── jev.rs               # Jev HTTP client, concurrency, retries
│       └── search.rs            # score frames, rank, cutoff
├── .env.example                 # JEV_API_KEY=, JEV_MODEL=jev-latest
├── .gitignore                   # .env, target/, node_modules/, learner profile
├── package.json                 # Tauri CLI scripts
├── README.md                    # what it is, setup, run, demo
└── devpost/                     # planning docs
```

## External Services and Dependencies

### Jev (TypeSafe)
- **Endpoint:** `POST https://api.typesafe.ai/v1/systemone`
- **Headers:** `Authorization: Bearer $JEV_API_KEY`, `Content-Type: application/json`, `TypeSafe-Danger-Zone-Images: true`
- **Request (verified in build step 1):** the image goes in a top-level `assets` map as base64 JPEG, and `state` refers to it by name.
  ```json
  {
    "model": "jev-latest",
    "state": { "frame": "splitseek-frame" },
    "assets": {
      "splitseek-frame": { "type": "image", "format": "jpg", "data_base64": "<base64, no data: prefix>" }
    },
    "questions": {
      "match": {
        "type": "noul",
        "instructions": "Does the video frame `frame` show: street market at night?",
        "criteria": {
          "true": "The frame clearly shows this",
          "false": "The frame does not show this"
        }
      }
    }
  }
  ```
- **Response:** `{ "model": "jev-1.13.0", "answers": { "match": { "type": "noul", "noul": 0.87 } }, "usage": { "input_tokens": 740, ... } }`. `noul` runs from 0 (no) to 1 (yes) and is used as the confidence score.
- **Image limits:** PNG or JPEG; each side 280–1568 px; 78,400–2,458,624 total pixels; 16 MiB per image. Out-of-range images are rejected, not resized, so frames must stay inside these bounds.
- **Measured (build step 1, one 512×288 frame):** about 740 input tokens and 100–360 ms per question. `jev-latest` currently answers as `jev-1.13.0`. At about $42 per billion input tokens, that's roughly $0.00003 per frame. Rate limits aren't published (a 429 means back off). Request body up to 32 MiB.
- **Status:** image input is "danger zone" (experimental). Jev is early access.
- **Docs:** https://docs.typesafe.ai/api, https://docs.typesafe.ai/images.md

### ffmpeg
A local program, with no key or cost. Filters reference: https://ffmpeg.org/ffmpeg-filters.html

## Important Failure Modes
- **Jev rejects images or returns nonsense** → found in build step 1, before any UI exists. Try `jev-preview`. If both fail, switch to a different vision model to finish the proof (learner decision; model to be chosen at that point).
- **Rate limiting (429) during a search** → automatic backoff and retry. Frames that still fail are skipped, and the terminal log counts them.
- **ffmpeg can't decode a clip** (an odd codec) → skip it, keep going, and log the count in the terminal.
- **Missing `JEV_API_KEY`** → the search shows "Jev API key missing — add it to .env" instead of crashing.

## What Was Simplified and Why
- **One generic contrast and saturation boost** instead of camera-specific LUTs for Canon Log 3 and D-Log M. The footage comes from three cameras with two LOG profiles, and detecting each camera adds work. A per-camera LUT would need camera detection from metadata plus a `.cube` file per profile.
- **One Jev call per frame, every search** instead of a pre-built search index. This is the kernel: Jev's speed is what's being tested. Caching or indexing belongs with the full-library work.
- **Fixed sampling every 2 seconds** (learner choice) instead of scene detection. Simple, and timestamps within about 2 seconds are fine for editing.
- **Base64 thumbnails** instead of letting the window read frame files from disk. That's fine for dozens of rows. (Video preview does read the original clips, through `clip://`, limited to the prepared folder's clips.)
- **Fixed cutoff constant** instead of a settings screen (`prd.md > Deferred From the POC`).

## Decisions and Open Issues

### Decisions
- **Tauri 2 + Rust** (learner): wants to try Rust through the coding agent and accepts the learning curve.
- **Every 2 seconds** (learner): "even if it's off by one or two seconds, the user can catch that"; every 5 seconds might miss moments.
- **Fallback path** (learner): try `jev-preview`, don't contact TypeSafe, and use a different vision model only if all else fails.
- **Remember prepared clips** (learner, first build checkpoint): all clips live on one Passport drive that reads at ~40 MB/s, so the first preparation of a large folder takes ~20 minutes; per-clip caching makes later visits fast.
- **One row per stretch** (learner, first build checkpoint): see `prd.md > Product Decisions`.
- **Keep the generic boost** (default, first build checkpoint): on `flip/` boosted and raw LOG performed about the same; one Canon frame scored higher boosted (84% vs. 68%). The learner didn't ask to change it.
- **Local only, no deployment** (carried from scope and PRD): demo video plus public repo.
- **Generic boost instead of LUTs** (agent recommendation, presented to the learner): mixed cameras.
- **Plain HTML/JS frontend; recursive folder scan; 16 parallel Jev calls; 4 parallel ffmpeg jobs; 512px frames** (agent implementation details, derived from the decisions above and open to change).

### Learner uncertainty explored
- **"Is Tauri a language?"** Clarified: it's a framework that pairs a Rust engine with a web-page window, like a lighter Electron. The learner will see the split directly. `src/` is the window and `src-tauri/` is the engine.
- **Does the LOG boost actually help Jev?** Agreed small investigation: in an early build step, score the same 3–4 phrases against raw LOG frames and boosted frames from a few clips, and compare. Keep the boost (and tune it) only if it improves the right matches.

### Open issues
- **Jev image input format.** Resolved in build step 1 (see External Services).
- **Jev per-image cost and speed.** Single frames measured in step 1; whole-folder speed is measured by the search timing log.
- **Exact LOG profiles.** Expected to be Canon Log 3 (R50 V) and D-Log M (DJI). Confirm with ffprobe on real files. Only relevant if LUTs come later.
- **Cutoff value.** Tuned to 0.60 in build step 4; revisit if real searches show misses.
- **Whether Rust speed work matters.** Decided by the timing log, not assumed.
