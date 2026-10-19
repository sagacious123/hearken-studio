/**
 * No right-click menu (Reload, Inspect Element…) in the released app. Text fields keep theirs,
 * for copy and paste. `npm run tauri dev` keeps the menu, to inspect while developing.
 */
export function blockContextMenu() {
  if (import.meta.env.DEV) return;
  window.addEventListener("contextmenu", (e) => {
    const target = e.target instanceof Element ? e.target : null;
    if (target?.closest("input, textarea, [contenteditable='true']")) return;
    e.preventDefault();
  });
}
