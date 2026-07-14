import { open } from "@tauri-apps/plugin-dialog";

export async function pickAudio(): Promise<string | null> {
  const selected = await open({
    multiple: false,
    filters: [{ name: "Audio", extensions: ["mp3", "m4a", "aac", "wav", "ogg", "flac"] }],
  });
  return typeof selected === "string" ? selected : null;
}
