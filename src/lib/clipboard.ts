import { writeText } from "@tauri-apps/plugin-clipboard-manager";

/** Copies text via the native clipboard plugin, falling back to the web API. */
export async function copyText(text: string): Promise<boolean> {
  try {
    await writeText(text);
    return true;
  } catch {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch {
      return false;
    }
  }
}
