---
doc: scope
status: approved
---

# SplitSeek

A standalone tool that searches a folder of LOG footage by plain-language description and returns the matching clips with timestamps, powered by Jev AI.

## The Unique Kernel
Fast visual search across footage whose thumbnails all look the same. Jev AI is a decision model: it answers typed questions ("does this frame match *street market at night*?") instead of generating text, which the learner has found "so much faster" than LLMs like Claude or OpenAI's models. Each frame is converted from flat LOG to a viewable image before Jev sees it, so search works on footage that's hard to tell apart by eye.

## Who It's For
The learner: a traveler who shoots LOG footage, many clips per location, and edits in DaVinci Resolve (free version). Today, when they want to reuse a clip or part of one, they scrub through near-identical washed-out thumbnails to find it.

## The Core Loop
Point the tool at a folder of clips (e.g., from an SD card) → type a phrase like "street market at night" → get a list of matching clips, each with the timestamp where the moment appears → jump to that clip in Resolve. Repeat for each clip they need while editing.

## Inspiration & Identity
Not discussed yet; `3-prd` covers look and feel. Its job is to stay out of the way: a quick utility next to Resolve, not a replacement for it.

## Why This Matters to the Learner
"I just wanted to ease my workflow." Finding clips in similar-looking LOG footage is a real, recurring pain in their editing. It's also a showcase of disciplined, industry-standard agent workflow for their job search.

## What "Working" Looks Like
In a one-minute demo: point the tool at a real folder of the learner's LOG clips, run three or four different searches, and get back the right clip(s) with timestamps each time ("if there's one, there's one; if there's multiple, there's multiple").
The "oh, that's cool" beat is that it finds the right moment among clips that look nearly identical as thumbnails, and it does so quickly.

## The POC Boundary
- Works on **one folder** of the learner's real clips (e.g., one trip or location, roughly 20–50 clips with lookalikes).
- Pulls sample frames from each clip and applies a simple contrast boost or LOG-to-normal conversion to **small copies only**. Original files are read-only and never modified. The copies are kept in SplitSeek's own cache folder on the Mac, never beside the originals, so reopening a folder is fast (added at the first build checkpoint; see `prd.md > Product Decisions`).
- Free-text search phrase → Jev AI yes/no matching on frames → list of matching clips plus timestamps.
- Runs locally as a separate tool, outside Resolve.

## Later
- Scale to the full ~500 GB library, which will need pre-indexing. (Per-clip caching of extracted frames moved into the PoC at the first build checkpoint.)
- Speech search ("find where someone says…"), which needs a separate transcription tool because Jev doesn't process audio.
- Organize clips into folders by date range or location on request.
- Write results into Resolve's clip Keywords and Comments, e.g., through CSV metadata import.
- Rust or other speed work, if profiling shows it would help. To discuss in `4-spec`.

## Explicitly Cut
- **Resolve plugin.** The learner prefers a separate tool, and free Resolve limits external scripting.
- **Auto-sorting an SD card.** The learner can already do this; it's tedious, but less so than searching.
- **Whole-library processing in the PoC.** Extracting frames from 500 GB could take hours per run, which would slow every build-and-test cycle and make wrong results hard to diagnose.
