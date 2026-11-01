import { defineStore, acceptHMRUpdate } from "pinia";
import { ref } from "vue";

// ── Font family ───────────────────────────────────────────────────────────────
// "monospace" preserves the app's original look. The chosen value is applied as
// the CSS variable `--app-font-family`, consumed by base.css / theme.css.

export const DEFAULT_FONT_FAMILY = "monospace";

export const FONT_FAMILY_OPTIONS: { value: string; label: string }[] = [
  { value: "monospace", label: "Monospace (default)" },
  { value: "Consolas, monospace", label: "Consolas" },
  { value: "'Cascadia Code', 'Cascadia Mono', monospace", label: "Cascadia Code" },
  { value: "'Courier New', Courier, monospace", label: "Courier New" },
  { value: "'Lucida Console', monospace", label: "Lucida Console" },
  { value: "Expressway, 'Segoe UI', sans-serif", label: "Expressway" },
  { value: "'Segoe UI', system-ui, sans-serif", label: "Segoe UI" },
  { value: "system-ui, sans-serif", label: "System Default (sans-serif)" },
  { value: "Arial, Helvetica, sans-serif", label: "Arial" },
  { value: "Georgia, 'Times New Roman', serif", label: "Georgia (serif)" },
  { value: "Verdana, Geneva, sans-serif", label: "Verdana" },
];

export function applyFontFamily(fontFamily: string) {
  const value = fontFamily && fontFamily.trim().length > 0 ? fontFamily : DEFAULT_FONT_FAMILY;
  document.documentElement.style.setProperty("--app-font-family", value);
}

// ── UI scale ──────────────────────────────────────────────────────────────────
// Whole-app zoom as a percentage (100 = native). Applied via CSS `zoom` on the
// document root, scaling text, spacing and icons uniformly — handy on 4K/high-DPI.

export const DEFAULT_UI_SCALE = 100;
export const MIN_UI_SCALE = 50;
export const MAX_UI_SCALE = 200;

export function applyUiScale(percent: number) {
  const clamped = Math.min(MAX_UI_SCALE, Math.max(MIN_UI_SCALE, percent || DEFAULT_UI_SCALE));
  // `zoom` takes a unitless multiplier (1 = 100%).
  (document.documentElement.style as CSSStyleDeclaration & { zoom: string }).zoom = `${clamped / 100}`;
}

// ── Store (localStorage-backed) ───────────────────────────────────────────────

const LS_KEY = "pgmv:settings";

interface PersistedSettings {
  uiFontFamily: string;
  uiScale: number;
  autoSpin: boolean;
}

function load(): PersistedSettings {
  try {
    const raw = localStorage.getItem(LS_KEY);
    if (raw) {
      const p = JSON.parse(raw) as Partial<PersistedSettings>;
      return {
        uiFontFamily: p.uiFontFamily ?? DEFAULT_FONT_FAMILY,
        uiScale: p.uiScale ?? DEFAULT_UI_SCALE,
        autoSpin: typeof p.autoSpin === "boolean" ? p.autoSpin : true,
      };
    }
  } catch {
    // Corrupt/absent — fall back to defaults.
  }
  return { uiFontFamily: DEFAULT_FONT_FAMILY, uiScale: DEFAULT_UI_SCALE, autoSpin: true };
}

export const useSettingsStore = defineStore("settings", () => {
  const initial = load();
  const uiFontFamily = ref(initial.uiFontFamily);
  const uiScale = ref(initial.uiScale);
  const autoSpin = ref(initial.autoSpin);

  function persist() {
    try {
      localStorage.setItem(
        LS_KEY,
        JSON.stringify({ uiFontFamily: uiFontFamily.value, uiScale: uiScale.value, autoSpin: autoSpin.value }),
      );
    } catch {
      // Storage unavailable — settings just won't persist.
    }
  }

  function setFontFamily(value: string) {
    uiFontFamily.value = value;
    applyFontFamily(value);
    persist();
  }

  function setAutoSpin(enabled: boolean) {
    autoSpin.value = enabled;
    persist();
  }

  function setUiScale(percent: number) {
    const clamped = Math.min(MAX_UI_SCALE, Math.max(MIN_UI_SCALE, percent || DEFAULT_UI_SCALE));
    uiScale.value = clamped;
    applyUiScale(clamped);
    persist();
  }

  /** Live-apply a scale during a slider drag without persisting yet. */
  function previewUiScale(percent: number) {
    applyUiScale(percent);
  }

  function resetUiScale() {
    setUiScale(DEFAULT_UI_SCALE);
  }

  /** Apply the persisted settings to the document (call once on app start). */
  function applyAll() {
    applyFontFamily(uiFontFamily.value);
    applyUiScale(uiScale.value);
  }

  return {
    uiFontFamily,
    uiScale,
    autoSpin,
    setAutoSpin,
    setFontFamily,
    setUiScale,
    previewUiScale,
    resetUiScale,
    applyAll,
  };
});

if (import.meta.hot) {
  import.meta.hot.accept(acceptHMRUpdate(useSettingsStore, import.meta.hot));
}
