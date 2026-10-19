---
doc: prd
status: approved
---

# SplitSeek — Product Requirements

A dark, Resolve-style desktop utility that sits beside DaVinci Resolve. Point it at a folder of LOG clips, type a phrase, and it ranks the moments that match, with timestamps and confidence. Built for the learner, a traveler who edits LOG footage in Resolve (free).
Source: `scope.md > The Core Loop`, `scope.md > The Unique Kernel`.

Named **SplitSeek**, a play on the video-editing term "split" (learner choice; "LogFinder" was rejected).

## The Core Journey
1. **Open the app.** A single dark window shows a prompt to choose a folder, with a "Choose folder" button and a drag-and-drop area.
2. **Choose a folder.** Either browse to it or drag the folder in, for example from the desktop or an SD card.
3. **Preparation.** The app tells the learner it's pulling frames from the clips and shows progress. Search is unavailable until preparation finishes.
4. **Search.** Type a phrase ("dog running in a field", "street market at night") and submit.
5. **Ranked results.** Each matching moment appears as its own row, ranked highest confidence first. Only moments at or above the cutoff appear by default.
6. **Act on a result.** Click a row to reveal that clip in Finder, then bring it into Resolve.
7. **Repeat.** Search again on the same folder with no re-preparation wait. This is where the speed shows.

Success: in a one-minute demo, three or four different searches on a real folder each surface the right moment(s) quickly (`scope.md > What "Working" Looks Like`).

## Screens and Layout
One window, with three states that share it:
- **Folder picker:** a choose-folder button and a drag-and-drop zone.
- **Preparing:** a message such as "Pulling frames from your clips…" with progress.
- **Search:** a search box at the top and a ranked results list below. A "Show more" control appears below the list when moments fell under the cutoff. There's also a way to choose a different folder.

The window should work comfortably beside Resolve, not full screen.

## Look and Feel
- Dark interface modeled on **DaVinci Resolve's** look. The learner uses Resolve and wants this to feel like part of the same setup. They also noted that iMovie and Premiere are dark.
- Clean, editor-tool feel: dense but readable, nothing flashy.
- Preview frames show the **original LOG frame**, flat and unconverted, so it matches what the learner sees in Resolve's media pool.

## Features and Behavior

### Choosing a Folder
- As the learner, I want to pick or drag in a folder of clips so I can search the footage I'm editing with.
  - [ ] Clicking "Choose folder" opens a folder browser, and selecting a folder starts preparation.
  - [ ] Dragging a folder onto the window starts preparation the same way.
  - [ ] Original video files are never modified, moved, or renamed (`scope.md > The POC Boundary`).

### Preparing Clips
- As the learner, I want a clear one-time wait, after which every search is fast.
  - [ ] While preparing, the app shows that it's pulling frames and how far along it is.
  - [ ] When preparation finishes, the search box becomes available.
  - [ ] Any brightened or converted frames used for matching are temporary and are cleaned up afterward (`scope.md > The POC Boundary`).

### Searching
- As the learner, I want to describe a moment in plain words and get the moments that match, so I don't have to scrub through lookalike thumbnails.
  - [ ] Typing a phrase and submitting it produces a results list.
  - [ ] Searches after the first run without re-preparing the folder.
  - [ ] Searching the same folder for three or four different subjects returns the right clips, judged by the learner on their own footage.

### Ranked Results
- As the learner, I want results ranked by confidence, with weak matches hidden, so a vague match in 70 out of 100 clips doesn't bury the real one.
  - [ ] Each row is one moment and shows the **file name**, the **timestamp** of the moment, the **confidence**, and a **preview of the matched frame** (original LOG). If one clip matches at several moments, each moment gets its own row.
  - [ ] Rows are sorted by confidence, highest first.
  - [ ] Moments below the cutoff are hidden by default.
  - [ ] A "Show more" control reveals the below-cutoff moments, still ranked, beneath the main results.
  - [ ] The preview frame must not noticeably slow the search. If it does, it's the first thing to drop.

### Reveal in Finder
- As the learner, I want to click a result and land on the file so I can bring it into Resolve.
  - [ ] Clicking a row opens Finder with that clip's file selected.

## States and Boundaries
- **First open:** the folder picker with its drag-and-drop zone. There are no results yet.
- **Preparing:** a progress message, with search unavailable.
- **Results found:** a ranked list of above-cutoff moments, plus "Show more" if anything fell below the cutoff.
- **Nothing above the cutoff:** a message along the lines of "We couldn't find anything above [cutoff], but here are some that came close," followed by the best below-cutoff moments, ranked.
- **No matches at all:** a plain "nothing found" message. *Assumption: this is the fallback when nothing is returned even below the cutoff.*
- **Folder has no videos:** "There are no videos in this folder." The learner can choose another folder.
- **Persistence:** none between sessions. Prepared frames last only while the app is open on that folder. Reopening the app, or adding new clips, means preparing fresh. The learner's reason: clips get reused across many projects, so remembering everything could get large, and a fresh start picks up new clips.

## Product Decisions
- **Separate app beside Resolve, not a plugin.** They'll keep it open while editing.
- **Folder via picker or drag-and-drop.** Dragging is easier when the folder is on the desktop.
- **Ranked by confidence with a cutoff, plus "Show more."** Showing every weak match would be too much, but near-misses should still be reachable.
- **One row per moment.** Each row is directly usable for finding a specific shot.
- **Preview shows the LOG frame, not the converted one.** It matches what the learner sees in Resolve.
- **Click reveals in Finder; no playback.** Simpler, and enough to get the clip into Resolve.
- **No memory between sessions.** Footage gets reused across projects, so caching everything isn't worth it now.
- **Dark, Resolve-inspired look.** It's the editor they use.
- *Assumption:* the cutoff is a fixed value tuned during the build on real footage (around 20–40%, per the learner's initial thinking), not a user-facing setting.

## What We're Building
- One dark window: choose or drag a folder → prepare with progress → search → ranked rows (file name, timestamp, confidence, LOG preview) → "Show more" for below-cutoff moments → click to reveal in Finder.
- The empty, no-videos, and nothing-above-cutoff states described above.
- Matching powered by Jev AI on contrast-boosted or converted temporary frames, with the original files left untouched.

## Deferred From the POC
- **Playback at the matched moment.** Playing high-bitrate LOG may be heavy, and reveal-in-Finder is enough for now.
- **Remembering prepared folders between sessions.** The learner chose a fresh preparation each time.
- **Adjustable cutoff setting.** Fixed and tuned during the build for now.

## Possible Later Enhancements
- Search across the full ~500 GB library with caching (`scope.md > Later`).
- Speech search, which needs transcription.
- Organize clips into folders by date or location.
- Write results into Resolve Keywords and Comments.
- Speed work such as Rust, if measurements justify it.

## Non-Goals
- Not a Resolve plugin, and it doesn't control Resolve (`scope.md > Explicitly Cut`).
- Doesn't sort or organize SD card contents (`scope.md > Explicitly Cut`).
- Never edits, color-grades, or re-encodes the original clips.

## Open Questions
- **Does Jev's vision output include a usable confidence score?** Must be answered in `4-spec`, because ranking and the cutoff depend on it. If it doesn't, the spec needs another way to rank.
- **How many frames to sample per clip.** This trades preparation time against timestamp precision. For `4-spec`.
- **The exact cutoff value.** Tune during the build.
