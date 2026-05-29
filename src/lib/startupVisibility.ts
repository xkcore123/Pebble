export const START_HIDDEN_TO_TRAY_KEY = "pebble-start-hidden-to-tray";

export function readStartHiddenToTrayPreference(storage: Storage = localStorage): boolean {
  return storage.getItem(START_HIDDEN_TO_TRAY_KEY) === "true";
}

export function shouldShowMainWindowOnStartup(storage: Storage = localStorage): boolean {
  return !readStartHiddenToTrayPreference(storage);
}
