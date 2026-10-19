# SplitSeek

Search a folder of LOG footage by describing a moment in plain words.

LOG footage is flat and grey, so dozens of clips from one trip look nearly identical as thumbnails. SplitSeek is a small standalone desktop app you keep open next to whatever video editor you use; it doesn't need or connect to one. You point it at a folder of clips, type something like "a waterfall" or "llamas", and it ranks the moments that match, with timestamps, confidence, and a video preview that starts at the matched moment.

Matching is done by [Jev](https://docs.typesafe.ai) from TypeSafe, a decision model that answers yes/no questions about images. SplitSeek asks it one question per sampled frame: "Does this frame show: *a waterfall*?"

> Proof of concept, built for the Devpost *Build With AI: Basics* hackathon. macOS only.

## What it does

1. **Choose or drag in a folder.** SplitSeek finds the videos (`.mp4`, `.mov`, `.m4v`, including inside subfolders like `DCIM/…`) and pulls one frame every 2 seconds with ffmpeg.
2. **Search.** Type what you're looking for and press Return or **Search**. Every frame is scored by Jev, 64 questions at a time.
3. **Ranked results.** Back-to-back matching frames of a clip become one row (e.g. `0:04–0:26 · best 0:12`), ranked by confidence. Weaker matches (below 60%) wait behind **Show more**.
4. **Preview.** Click a row to play the original clip from its best moment.
5. **Get the clip.** Use **⋯ → Show in Finder**, then drag it into your editor.

Prepared frames are remembered, so reopening a folder is nearly instant and only new or changed clips are prepared again.

## Your footage is safe

- Original clips are **only ever read**. Nothing is written, moved, or renamed in your folders.
- Sampled frames are small JPEGs stored in `~/Library/Caches/com.splitseek.app/frames/` (about 22 MB for 57 clips). **Forget prepared frames** at the bottom of the window deletes them all.
- Frames are sent to Jev for scoring, so a search sends low-resolution stills of your footage to TypeSafe's API.
- The video preview can only read clips in the folder you prepared.

## Requirements

- macOS on Apple Silicon (built and tested on macOS 15)
- [Node.js](https://nodejs.org) and npm
- Rust, installed with [rustup](https://www.rust-lang.org/tools/install)
- [ffmpeg](https://ffmpeg.org) (including `ffprobe`) on your PATH, e.g. `brew install ffmpeg`
- A Jev API key from [TypeSafe](https://docs.typesafe.ai) with image ("danger zone") access

If you installed `rustup` with Homebrew, `cargo` isn't on your PATH by default. Add this to `~/.zshrc` and open a new terminal tab:

```bash
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
```

## Setup

```bash
git clone <this repository> splitseek
cd splitseek
npm install
cp .env.example .env
```

Open `.env` and paste your key after `JEV_API_KEY=`. `.env` is gitignored and never committed.

## Run

```bash
npm run tauri dev
```

The first run compiles the Rust engine, which takes a few minutes. Then the SplitSeek window opens.

Preparation speed depends on where your clips are. From a USB drive it is limited by the drive (about 18 minutes for 43 GB from one that reads at 40 MB/s). From the Mac's internal disk it is much faster. Either way it happens once per clip.

## How it works

SplitSeek is a [Tauri 2](https://v2.tauri.app) app: a plain HTML/CSS/JavaScript window (`src/`) and a Rust engine (`src-tauri/`).

```
Window (src/)                    Engine (src-tauri/src/)               Outside
─────────────                    ───────────────────────               ───────
pick / drag folder ──prepare──▶  scan.rs   find the videos
progress bar      ◀──events───   frames.rs ffmpeg → cached JPEGs
type, Search      ──search────▶  search.rs one question per frame ──▶ Jev API
ranked rows       ◀──results──   group into stretches, rank        ◀── score 0–1
click a row       ──clip://───▶  clip_protocol.rs (video, in pieces)
⋯ Show in Finder  ──reveal────▶  open -R <clip>                     ──▶ Finder
```

- **`scan.rs`** finds video files, skipping hidden files, macOS `._` sidecars, and DJI `.LRF` proxies.
- **`frames.rs`** runs one ffmpeg command per clip, 4 clips at a time. It decodes only keyframes on the Mac's video hardware (about 30× faster than a full decode on 4K HEVC) and saves a flat LOG frame for the preview plus a contrast-boosted copy for Jev. Each clip's frames are cached under a key built from its path, size, and modified time.
- **`jev.rs`** sends a frame to Jev and retries when it's rate-limited.
- **`search.rs`** scores every frame, joins back-to-back matches into stretches, sorts them, and splits them at the 60% cutoff.
- **`clip_protocol.rs`** serves video to the preview player. Tauri's built-in file access labelled uppercase `.MP4` files as `text/html`, which played audio over a black picture.

The design and the decisions behind it are in [`devpost/`](devpost): `scope.md`, `prd.md`, `spec.md`, and the build log in `checklist.md`.

## Command-line tools

These were used to build and tune SplitSeek before the window existed. They are handy for checking Jev directly. Run them from `src-tauri/`.

```bash
# Score one image against one or more phrases
cargo run --example jev_probe -- frame.jpg "a waterfall" "a city street"
```

```bash
# Prepare a folder, then search it from the terminal (interactive if no phrases are given)
cargo run --example search_cli -- ~/Desktop/clips "a waterfall" --compare
```

`--compare` scores the boosted and raw LOG frames side by side, and `--forget` clears the frame cache first.

## Tests

```bash
cd src-tauri && cargo test
```

The tests cover folder scanning, timestamps, ranking and stretches, the frame cache (this one needs ffmpeg), the Jev request format, and video byte ranges.

## Limitations

- macOS only (Finder reveal, VideoToolbox decoding, the cache location).
- One generic contrast boost for all cameras rather than per-camera LOG conversion.
- Every search scores every frame. That's about 2 seconds for 637 frames; a whole 500 GB library would need an index.
- Timestamps are accurate to about 2 seconds.
- No audio or speech search.

## License

MIT. See [LICENSE](LICENSE).
