import init from "/assets/admin_ui.js";

const wasmUrl = new URL("/assets/admin_ui_bg.wasm", `${location.protocol}//${location.host}`);

init({ module_or_path: wasmUrl }).catch((error) => {
  console.error("Failed to initialize Graphile Worker Admin UI", error);
});
