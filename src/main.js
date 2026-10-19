// The window: shows one of three states (picker, preparing, search) and talks to
// the Rust engine through Tauri commands and events.

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { open } = window.__TAURI__.dialog;
const { getCurrentWebview } = window.__TAURI__.webview;

const $ = (id) => document.getElementById(id);
const STATES = ["picker", "preparing", "search"];
let current = "picker";

function show(state) {
  current = state;
  for (const s of STATES) $(s).hidden = s !== state;
  $("change-folder").hidden = state !== "search";
  if (state === "picker") $("folder-name").textContent = "";
  if (state === "search") $("search-input").focus();
}

function showPickerError(message) {
  $("picker-error").textContent = message;
  $("picker-error").hidden = !message;
}

function plural(n, word) {
  return `${n.toLocaleString()} ${word}${n === 1 ? "" : "s"}`;
}

async function prepare(path) {
  if (current === "preparing") return;
  showPickerError("");
  $("progress-fill").style.width = "0";
  $("progress-count").textContent = "";
  $("progress-clip").textContent = "Looking for videos…";
  show("preparing");

  try {
    const s = await invoke("prepare_folder", { path });
    $("folder-name").textContent = s.folderName;
    const parts = [plural(s.clipCount, "clip"), plural(s.frameCount, "frame")];
    if (s.rememberedCount > 0) parts.push(`${s.rememberedCount.toLocaleString()} remembered`);
    if (s.skippedCount > 0) parts.push(`${s.skippedCount} couldn't be read`);
    $("prepared-line").textContent = parts.join(" · ");
    $("results").replaceChildren();
    $("results-status").textContent = "";
    $("search-input").value = "";
    clearPreview();
    show("search");
  } catch (message) {
    show("picker");
    showPickerError(String(message));
  }
}

// ── Search ──────────────────────────────────────────────────────────────

let searchId = 0; // ignores a slow search's results if a newer one has started

function formatTime(sec) {
  return `${Math.floor(sec / 60)}:${String(sec % 60).padStart(2, "0")}`;
}

function formatRange(start, end) {
  return start === end ? formatTime(start) : `${formatTime(start)}–${formatTime(end)}`;
}

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

// ── Preview ─────────────────────────────────────────────────────────────

function clearPreview() {
  const player = $("player");
  player.pause();
  delete player.dataset.clip;
  player.removeAttribute("src");
  player.load();
  player.hidden = true;
  $("preview-empty").hidden = false;
  $("preview-empty").textContent = "Click a result to preview it.";
  $("preview-caption").textContent = "";
}

/** Plays the original clip from the row's best moment. */
function preview(row, rowEl) {
  document.querySelectorAll(".row.selected").forEach((r) => r.classList.remove("selected"));
  rowEl.classList.add("selected");

  const player = $("player");
  // Served by the engine's clip:// channel (see clip_protocol.rs).
  const src = `clip://localhost/${encodeURIComponent(row.clipPath)}`;
  const startAt = () => {
    player.currentTime = row.bestSec;
    player.play().catch(() => {});
  };
  $("preview-empty").hidden = true;
  player.hidden = false;
  $("preview-caption").textContent = `${row.clipName} · from ${formatTime(row.bestSec)}`;
  if (player.dataset.clip === row.clipPath) {
    startAt();
  } else {
    player.dataset.clip = row.clipPath;
    player.src = src;
    player.addEventListener("loadedmetadata", startAt, { once: true });
  }
}

// ── "⋯" menu ────────────────────────────────────────────────────────────

let openMenu = null;

function closeMenu() {
  openMenu?.remove();
  openMenu = null;
}

function showMenu(row, anchor) {
  closeMenu();
  const menu = el("div", "menu");
  const finder = el("button", "menu-item", "Show in Finder");
  finder.addEventListener("click", async () => {
    closeMenu();
    try {
      await invoke("reveal_in_finder", { path: row.clipPath });
    } catch (message) {
      $("results-status").textContent = String(message);
    }
  });
  menu.append(finder);
  anchor.parentElement.append(menu);
  openMenu = menu;
  finder.focus();
}

// ── Results ─────────────────────────────────────────────────────────────

function renderRow(row) {
  const wrap = el("div", "row-wrap");
  const button = el("button", "row");
  button.title = `Preview ${row.clipName}`;
  button.addEventListener("click", () => preview(row, button));

  const thumb = el("img", "thumb");
  thumb.src = row.preview;
  thumb.alt = "";

  const info = el("div", "row-info");
  info.append(el("div", "row-name", row.clipName));
  const time = el("div", "row-time muted");
  time.textContent =
    row.startSec === row.endSec
      ? formatTime(row.bestSec)
      : `${formatRange(row.startSec, row.endSec)} · best ${formatTime(row.bestSec)}`;
  info.append(time);

  const pct = Math.round(row.score * 100);
  const confidence = el("div", "row-confidence");
  confidence.append(el("span", "pct", `${pct}%`));
  const bar = el("div", "score-bar");
  const fill = el("div", "score-bar-fill");
  fill.style.width = `${pct}%`;
  bar.append(fill);
  confidence.append(bar);

  button.append(thumb, info, confidence);

  const more = el("button", "row-more", "⋯");
  more.title = "More";
  more.setAttribute("aria-label", `More for ${row.clipName}`);
  more.addEventListener("click", (e) => {
    e.stopPropagation();
    if (openMenu && openMenu.parentElement === wrap) closeMenu();
    else showMenu(row, more);
  });

  wrap.append(button, more);
  return wrap;
}

function renderResults(phrase, r) {
  const results = $("results");
  const cutoffPct = Math.round(r.cutoff * 100);
  const list = el("div", "rows");
  const nearList = el("div", "rows");
  r.above.forEach((row) => list.append(renderRow(row)));
  r.below.forEach((row) => nearList.append(renderRow(row)));

  let status = `${r.above.length} ${r.above.length === 1 ? "match" : "matches"} · ${(r.elapsedMs / 1000).toFixed(1)} s`;
  if (r.framesFailed > 0) status += ` · ${plural(r.framesFailed, "frame")} couldn't be checked`;

  if (r.above.length === 0 && r.below.length === 0) {
    results.replaceChildren(el("p", "message", `Nothing found for “${phrase}”.`));
  } else if (r.above.length === 0) {
    results.replaceChildren(
      el("p", "message", `We couldn't find anything above ${cutoffPct}%, but here are some that came close.`),
      nearList,
    );
  } else if (r.below.length === 0) {
    results.replaceChildren(list);
  } else {
    const more = el("button", "show-more link", `Show more (${r.below.length})`);
    nearList.hidden = true;
    more.addEventListener("click", () => {
      nearList.hidden = !nearList.hidden;
      more.textContent = nearList.hidden ? `Show more (${r.below.length})` : `Show fewer`;
    });
    results.replaceChildren(list, more, nearList);
  }
  if (r.above.length === 0) status = `Nothing above ${cutoffPct}% · ${(r.elapsedMs / 1000).toFixed(1)} s`;
  $("results-status").textContent = status;
  results.scrollTop = 0;
}

async function runSearch(phrase) {
  phrase = phrase.trim();
  if (!phrase) return;
  const id = ++searchId;
  closeMenu();
  // A new search stops the preview, so playback doesn't stutter alongside the search.
  clearPreview();
  $("results-status").textContent = `Searching for “${phrase}”…`;
  $("results").classList.add("dim");
  $("search-button").disabled = true;
  try {
    const r = await invoke("search", { phrase });
    if (id !== searchId) return;
    renderResults(phrase, r);
  } catch (message) {
    if (id !== searchId) return;
    $("results").replaceChildren();
    $("results-status").textContent = String(message);
  } finally {
    if (id === searchId) {
      $("results").classList.remove("dim");
      $("search-button").disabled = false;
    }
  }
}

// ── Folder ──────────────────────────────────────────────────────────────

async function chooseFolder() {
  const path = await open({ directory: true, multiple: false, title: "Choose a folder of clips" });
  if (path) prepare(path);
}

window.addEventListener("DOMContentLoaded", async () => {
  $("choose-folder").addEventListener("click", chooseFolder);
  $("change-folder").addEventListener("click", () => show("picker"));

  $("forget").addEventListener("click", async () => {
    if (current === "preparing") return;
    await invoke("forget_prepared_frames");
    show("picker");
    showPickerError("");
  });

  $("player").addEventListener("error", () => {
    if (!$("player").getAttribute("src")) return;
    $("player").hidden = true;
    $("preview-empty").hidden = false;
    $("preview-empty").textContent = "This clip can't be previewed here. Use ⋯ → Show in Finder.";
  });
  document.addEventListener("click", (e) => {
    if (openMenu && !openMenu.contains(e.target)) closeMenu();
  });
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") closeMenu();
  });

  $("search-form").addEventListener("submit", (e) => {
    e.preventDefault();
    runSearch($("search-input").value);
  });

  await listen("prepare-progress", ({ payload }) => {
    const { done, total, clip } = payload;
    $("progress-fill").style.width = `${(done / total) * 100}%`;
    $("progress-count").textContent = `${done} / ${total}`;
    $("progress-clip").textContent = clip;
  });

  // Dragging a folder anywhere onto the window, while the picker is showing.
  await getCurrentWebview().onDragDropEvent(({ payload }) => {
    if (current !== "picker") return;
    const zone = $("drop-zone");
    if (payload.type === "enter" || payload.type === "over") zone.classList.add("over");
    else zone.classList.remove("over");
    if (payload.type === "drop" && payload.paths.length > 0) prepare(payload.paths[0]);
  });
});
