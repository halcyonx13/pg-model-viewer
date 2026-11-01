// Types for the 3D Model Viewer feature. Field names are snake_case to match
// the Rust command payloads on the wire (same convention as ItemInfo).

export interface ModelViewerStatus {
  game_bundles_dir: string | null;
  cache_ready: boolean;
  cache_dir: string;
  appearance_count: number;
  material_count: number;
  weapon_count: number;
  source_bundle: string | null;
}

export interface ResolvedSlot {
  slot: string;
  mesh_key: string | null;
  material_key: string | null;
  dye_order: string | null;
  dyeable: boolean;
  /** Relative glb path in the model cache, or null if not extracted. */
  mesh_file: string | null;
  /** label → relative png filename (skin/mask1..3/normal/metallic/...). */
  textures: Record<string, string> | null;
  dye_channels: number;
  /** e.g. { _Color0: [r,g,b,a], _Color1: [...], ... } */
  default_colors: Record<string, number[]> | null;
  /** Weapon/held prop — oriented by bounding box rather than the body rotation. */
  is_weapon: boolean;
}

export interface ResolvedItemAppearance {
  item_id: number;
  item_name: string;
  sex: string;
  source_field: string;
  slots: ResolvedSlot[];
  flags: Record<string, string>;
  renderable: boolean;
  not_dyeable_keyword: boolean;
}

export interface BrowsableItem {
  id: number;
  name: string;
  icon_id: number | null;
  equip_slot: string | null;
  /** True if this item resolves to a mesh in the cache (many cosmetics don't). */
  has_model: boolean;
  invisible: boolean;
  not_dyeable: boolean;
  /** Raw appearance string — items sharing it render identically (group key). */
  appearance_key: string | null;
}

/** One distinct model in the picker: all items sharing an appearance string. */
export interface ItemGroup {
  /** Representative item (shortest name — usually the base tier). */
  rep: BrowsableItem;
  /** Every member item name (for search + tooltip). */
  names: string[];
  /** Member item ids (for chosen-state checks against the loadout). */
  ids: number[];
}

/** An item chosen for a loadout slot. */
export interface LoadoutEntry {
  ref: string;
  name: string;
  icon_id: number | null;
}

/** Per-slot dye: directive-slot name → channel (1–3) → "#rrggbb". */
export type DyeMap = Record<string, Record<number, string>>;

/** One usable in-game dye (item with the `Dye` keyword + `DyeColor`). */
export interface DyeInfo {
  name: string;
  internal_name: string | null;
  /** Bare hex from the game data, e.g. "00BFFF". */
  color: string;
  icon_id: number | null;
}

export interface ProfileDye {
  name: string;
  color: string;
}

/** An ordered palette of exactly three dyes, independent of equipment. */
export interface DyeProfile {
  id: string;
  name: string;
  dyes: [ProfileDye, ProfileDye, ProfileDye];
}

/** A saved, reloadable loadout (items + dye + sex). Persisted to localStorage. */
export interface LoadoutPreset {
  id: string;
  name: string;
  sex: "m" | "f";
  loadout: Record<string, LoadoutEntry>;
  dye: DyeMap;
}

/** Equipment slots the Model Viewer shows (those with a 3D model). */
export const VIEWER_SLOTS: { id: string; label: string }[] = [
  { id: "Head", label: "Head" },
  { id: "Chest", label: "Chest" },
  { id: "Legs", label: "Legs" },
  { id: "Hands", label: "Hands" },
  { id: "Feet", label: "Feet" },
  { id: "MainHand", label: "Main Hand" },
  { id: "OffHand", label: "Off Hand" },
];

export interface ExtractionProgress {
  stage: string;
  message: string;
  done: boolean;
  ok: boolean;
}

/** Equipment slots that have a visible 3D appearance. */
export const APPEARANCE_SLOTS = ["Head", "Chest", "Legs", "Hands", "Feet"] as const;
