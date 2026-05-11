import init from "./admin_ui.js";

const wasmUrl = new URL("./admin_ui_bg.wasm", import.meta.url);

init({ module_or_path: wasmUrl }).catch((error) => {
  console.error("Failed to initialize Graphile Worker Admin UI", error);
});
