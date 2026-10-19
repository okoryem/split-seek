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
    $("search-input").value = "";
    show("search");
  } catch (message) {
    show("picker");
    showPickerError(String(message));
  }
}

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

  // Search arrives in the next build step; for now the box just unlocks.
  $("search-form").addEventListener("submit", (e) => e.preventDefault());

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
