import i18n from "../i18n";

/** Clock time in the app language (24-hour for Russian), without seconds. */
export function formatClock(date: Date): string {
  return date.toLocaleTimeString(i18n.language, { hour: "2-digit", minute: "2-digit" });
}
