import { open } from "@tauri-apps/plugin-dialog";

export async function pickImage(): Promise<string | null> {
  const selected = await open({
    multiple: false,
    filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "webp"] }],
  });
  return typeof selected === "string" ? selected : null;
}
