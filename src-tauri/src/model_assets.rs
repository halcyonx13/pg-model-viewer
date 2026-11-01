//! Model Viewer asset backend.
//!
//! Locates the user's local Project: Gorgon install, drives the offline
//! extractor (`tools/model_extractor/extract.py`) that converts Unity bundles
//! into a web-renderable cache (`{appdata}/models/`), and exposes commands the
//! frontend three.js viewer uses to resolve an item to its mesh/material/dye
//! assets.
//!
//! No game art ships in the repo or the app — everything is extracted from the
//! player's own install into their local cache. See the plan and
//! `game_data::appearance` for the item→appearance mapping.

use crate::cdn_commands::GameDataState;
use crate::game_data::appearance::{self, AppearanceDirective};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager, State};

/// Steam application id for Project: Gorgon.
const PG_STEAM_APPID: &str = "342940";
/// Catalog schema this app expects (must match extract.py's CATALOG_SCHEMA).
/// An older cache reports `cache_ready=false` so the UI prompts a re-extract.
const EXPECTED_CATALOG_SCHEMA: u64 = 6;
/// Relative path from a game install root to the addressable bundle dir.
const BUNDLE_SUBPATH: &str =
    "WindowsPlayer_Data/StreamingAssets/aa/StandaloneWindows64";

// ── Cache location ────────────────────────────────────────────────────────────

/// `{appdata}/models/` — where the extractor writes glb + textures + catalog.
pub fn model_cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve app data dir: {e}"))?;
    let dir = base.join("models");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create models dir: {e}"))?;
    Ok(dir)
}

fn catalog_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(model_cache_dir(app)?.join("catalog.json"))
}

// ── Install detection ─────────────────────────────────────────────────────────

/// Candidate Steam roots to probe for `libraryfolders.vdf`.
fn steam_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        if cfg!(target_os = "macos") {
            roots.push(home.join("Library/Application Support/Steam"));
        } else if cfg!(target_os = "linux") {
            roots.extend([
                home.join(".steam/steam"),
                home.join(".local/share/Steam"),
                home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
            ]);
        }
    }
    for var in ["ProgramFiles(x86)", "ProgramFiles", "ProgramW6432"] {
        if let Ok(p) = std::env::var(var) {
            roots.push(PathBuf::from(p).join("Steam"));
        }
    }
    // Common non-default drives.
    for drive in ["C", "D", "E", "G"] {
        roots.push(PathBuf::from(format!("{drive}:\\SteamLibrary")));
        roots.push(PathBuf::from(format!("{drive}:\\Steam")));
    }
    roots
}

/// Parse the `"path"` values out of a Steam `libraryfolders.vdf`.
fn parse_library_paths(vdf: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for line in vdf.lines() {
        let line = line.trim();
        // e.g.  "path"		"C:\\Program Files (x86)\\Steam"
        if let Some(rest) = line.strip_prefix("\"path\"") {
            if let Some(start) = rest.find('"') {
                let after = &rest[start + 1..];
                if let Some(end) = after.find('"') {
                    let raw = &after[..end];
                    out.push(PathBuf::from(raw.replace("\\\\", "\\")));
                }
            }
        }
    }
    out
}

/// Try to locate the addressable bundle directory of the local PG install.
/// Checks an explicit override first, then Steam library folders.
fn bundles_in_install(root: &Path) -> Option<PathBuf> {
    if !cfg!(windows) {
        return native_bundles_in_install(root);
    }
    let bundles = root.join(BUNDLE_SUBPATH);
    if bundles.is_dir() && (!cfg!(windows) || root.join("WindowsPlayer.exe").is_file()) {
        Some(bundles)
    } else {
        None
    }
}

fn child_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<_> = std::fs::read_dir(root).into_iter().flatten()
        .filter_map(Result::ok).map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    dirs
}

fn contains_gear_bundle(root: &Path) -> bool {
    std::fs::read_dir(root).into_iter().flatten().filter_map(Result::ok)
        .any(|e| e.path().is_file() && e.file_name().to_string_lossy().contains("newmodel_assets_all"))
}

// Use actual extracted-bundle names rather than guessing the native platform
// directory's spelling. Accept install roots, .app bundles, aa, or bundle dirs.
fn native_bundles_in_install(root: &Path) -> Option<PathBuf> {
    let mut data_roots = vec![root.to_path_buf()];
    data_roots.extend(child_dirs(root).into_iter().filter(|p| {
        let name = p.file_name().unwrap_or_default().to_string_lossy();
        name.ends_with("_Data") || name.ends_with(".app")
    }));
    for data in data_roots {
        for candidate in [
            data.clone(),
            data.join("StreamingAssets/aa"),
            data.join("Contents/Resources/Data/StreamingAssets/aa"),
            data.join("Contents/Data/StreamingAssets/aa"),
        ] {
            if contains_gear_bundle(&candidate) { return Some(candidate); }
            if let Some(bundles) = child_dirs(&candidate).into_iter().find(|p| contains_gear_bundle(p)) {
                return Some(bundles);
            }
        }
    }
    None
}

pub fn locate_bundles_dir(explicit: Option<&str>) -> Option<PathBuf> {
    // 1. Explicit override — accept either the install root or the bundle dir.
    if let Some(e) = explicit.filter(|s| !s.trim().is_empty()) {
        let p = PathBuf::from(e.trim().trim_matches('"'));
        if !cfg!(windows) {
            return native_bundles_in_install(&p);
        }
        // Normalize the legacy bundle-dir/aa-dir inputs back to the install
        // root so Windows always validates the player executable as well.
        let root = if p.ends_with("StandaloneWindows64") {
            p.ancestors().nth(4)?
        } else if p.ends_with("aa") {
            p.ancestors().nth(3)?
        } else {
            &p
        };
        // Never silently substitute another install for an invalid user path.
        return bundles_in_install(root);
    }

    if cfg!(windows) {
        if let Some(bundles) = bundles_in_install(Path::new(
            r"C:\Program Files (x86)\Steam\steamapps\common\Project Gorgon",
        )) {
            return Some(bundles);
        }
    }

    // 2. Steam library detection.
    let mut library_paths: Vec<PathBuf> = Vec::new();
    for root in steam_roots() {
        let vdf = root.join("steamapps").join("libraryfolders.vdf");
        if let Ok(contents) = std::fs::read_to_string(&vdf) {
            library_paths.extend(parse_library_paths(&contents));
        }
        // The root itself is always an implicit library.
        library_paths.push(root);
    }
    library_paths.sort();
    library_paths.dedup();

    for lib in library_paths {
        let root = lib
            .join("steamapps")
            .join("common")
            .join("Project Gorgon");
        if let Some(bundles) = bundles_in_install(&root) {
            return Some(bundles);
        }
    }

    let _ = PG_STEAM_APPID; // reserved for future manifest-based detection
    None
}

// ── Catalog access ────────────────────────────────────────────────────────────

fn load_catalog(app: &AppHandle) -> Result<serde_json::Value, String> {
    let path = catalog_path(app)?;
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read catalog ({}): {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("catalog.json parse error: {e}"))
}

#[derive(Serialize)]
pub struct ModelViewerStatus {
    pub platform: &'static str,
    /// Absolute bundle dir if a local install was found.
    pub game_bundles_dir: Option<String>,
    /// True if `{appdata}/models/catalog.json` exists.
    pub cache_ready: bool,
    /// Absolute cache root (for building asset URLs on the frontend).
    pub cache_dir: String,
    pub appearance_count: usize,
    pub material_count: usize,
    pub weapon_count: usize,
    /// Source bundle recorded in the catalog (for staleness checks).
    pub source_bundle: Option<String>,
}

#[tauri::command]
pub async fn model_viewer_status(
    app: AppHandle,
    game_dir: Option<String>,
) -> Result<ModelViewerStatus, String> {
    let cache_dir = model_cache_dir(&app)?;
    let bundles = locate_bundles_dir(game_dir.as_deref());
    let (cache_ready, appearance_count, material_count, weapon_count, source_bundle) =
        match load_catalog(&app) {
            Ok(cat) => {
                let count = |k: &str| cat.get(k).and_then(|v| v.as_object()).map_or(0, |o| o.len());
                // An older-schema cache has stale meshes — treat as not ready
                // so the UI offers re-extraction (the extractor force-refreshes).
                let schema_ok = cat.get("schema").and_then(|v| v.as_u64())
                    == Some(EXPECTED_CATALOG_SCHEMA);
                (
                    schema_ok,
                    count("appearances"),
                    count("materials"),
                    count("weapons"),
                    cat.get("sourceBundle")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                )
            }
            Err(_) => (false, 0, 0, 0, None),
        };
    Ok(ModelViewerStatus {
        platform: std::env::consts::OS,
        game_bundles_dir: bundles.map(|p| p.to_string_lossy().to_string()),
        cache_ready,
        cache_dir: cache_dir.to_string_lossy().to_string(),
        appearance_count,
        material_count,
        weapon_count,
        source_bundle,
    })
}

/// Absolute cache root; the frontend joins `meshes/…`/`textures/…` and passes
/// the result to Tauri's `convertFileSrc`.
#[tauri::command]
pub async fn model_cache_root(app: AppHandle) -> Result<String, String> {
    Ok(model_cache_dir(&app)?.to_string_lossy().to_string())
}

/// Raw catalog.json (appearances / materials / weapons).
#[tauri::command]
pub async fn get_model_catalog(app: AppHandle) -> Result<serde_json::Value, String> {
    load_catalog(&app)
}

// ── Item → appearance resolution ──────────────────────────────────────────────

#[derive(Serialize)]
pub struct ResolvedSlot {
    pub slot: String,
    pub mesh_key: Option<String>,
    pub material_key: Option<String>,
    pub dye_order: Option<String>,
    pub dyeable: bool,
    /// Relative glb path in the cache, if the mesh was extracted.
    pub mesh_file: Option<String>,
    /// Relative texture paths (skin/mask1..3/normal/metallic), if a material matched.
    pub textures: Option<serde_json::Value>,
    pub dye_channels: u8,
    /// Default dye colors from the material (`_Color0.._Color3`).
    pub default_colors: Option<serde_json::Value>,
    /// True if the mesh is a weapon/held prop (not skinned body gear). The
    /// viewer orients these by bounding box (longest axis up) instead of the
    /// character Z-up→Y-up rotation.
    pub is_weapon: bool,
}

#[derive(Serialize)]
pub struct ResolvedItemAppearance {
    pub item_id: u32,
    pub item_name: String,
    pub sex: char,
    pub source_field: String,
    pub slots: Vec<ResolvedSlot>,
    pub flags: std::collections::BTreeMap<String, String>,
    /// True if at least one slot has a mesh present in the cache.
    pub renderable: bool,
    pub not_dyeable_keyword: bool,
}

fn join_catalog(cat: &serde_json::Value, d: &AppearanceDirective) -> ResolvedSlot {
    let lookup = |section: &str, key: &str| -> Option<serde_json::Value> {
        cat.get(section)
            .and_then(|s| s.get(key))
            .cloned()
    };

    // Mesh: appearances (skinned body gear) first, then weapons (held props).
    let gear_entry = d.mesh_key.as_ref().and_then(|k| lookup("appearances", k));
    let weapon_entry = d
        .mesh_key
        .as_ref()
        .filter(|_| gear_entry.is_none())
        .and_then(|k| lookup("weapons", k));
    let is_weapon = gear_entry.is_none() && weapon_entry.is_some();
    let mesh_file = gear_entry
        .as_ref()
        .or(weapon_entry.as_ref())
        .and_then(|e| e.get("meshFile").and_then(|v| v.as_str()).map(String::from));

    // Material: explicit ^Armor= key, else the appearance's defaultMaterial,
    // else a weapon's material.
    let mat_key = d.material_key.clone().or_else(|| {
        d.mesh_key.as_ref().and_then(|k| {
            lookup("appearances", k)
                .and_then(|e| e.get("defaultMaterial").and_then(|v| v.as_str()).map(String::from))
                .or_else(|| {
                    lookup("weapons", k)
                        .and_then(|e| e.get("material").and_then(|v| v.as_str()).map(String::from))
                })
        })
    });

    let mat = mat_key.as_ref().and_then(|k| lookup("materials", k));
    let textures = mat.as_ref().and_then(|m| m.get("textures").cloned());
    let dye_channels = mat
        .as_ref()
        .and_then(|m| m.get("dyeChannels").and_then(|v| v.as_u64()))
        .unwrap_or(0) as u8;
    let default_colors = mat.as_ref().and_then(|m| m.get("colors").cloned());

    ResolvedSlot {
        slot: d.slot.clone(),
        mesh_key: d.mesh_key.clone(),
        material_key: mat_key,
        dye_order: d.dye_order.clone(),
        dyeable: d.dyeable,
        mesh_file,
        textures,
        dye_channels,
        default_colors,
        is_weapon,
    }
}

/// Resolve any item reference to its 3D appearance, joined against the local
/// asset catalog. `sex` is `"m"` or `"f"` (defaults to male).
#[tauri::command]
pub async fn resolve_item_appearance(
    reference: String,
    sex: Option<String>,
    app: AppHandle,
    state: State<'_, GameDataState>,
) -> Result<Option<ResolvedItemAppearance>, String> {
    let sex_ch = sex
        .as_deref()
        .and_then(|s| s.chars().next())
        .filter(|c| *c == 'm' || *c == 'f')
        .unwrap_or('m');

    let data = state.read().await;
    let item = match data.resolve_item(&reference) {
        Some(i) => i,
        None => return Ok(None),
    };

    let parsed = match appearance::resolve_item_appearance(
        item.equip_appearance.as_deref(),
        item.equip_appearance2.as_deref(),
        sex_ch,
    ) {
        Some(p) => p,
        None => return Ok(None),
    };

    let cat = load_catalog(&app).unwrap_or_else(|_| serde_json::json!({}));
    let slots: Vec<ResolvedSlot> = parsed
        .directives
        .iter()
        .map(|d| join_catalog(&cat, d))
        .collect();
    let renderable = slots.iter().any(|s| s.mesh_file.is_some());

    Ok(Some(ResolvedItemAppearance {
        item_id: item.id,
        item_name: item.name.clone(),
        sex: sex_ch,
        source_field: parsed.source_field.to_string(),
        slots,
        flags: parsed.flags,
        renderable,
        not_dyeable_keyword: item.keywords.iter().any(|k| k == "NotDyeable"),
    }))
}

// ── Base body (Phase 2 paper doll) ────────────────────────────────────────────

/// Race prefixes to try when picking a base-body skin material, most-complete
/// first. `f` is the only prefix in `newmodel` with a full skin set including
/// `skin-face`; the others fill body/hands/feet. Eyes fall back to `x`.
const SKIN_RACE_ORDER: &[&str] = &["f", "x", "o", "r", "h", "e", "d"];

/// First candidate material key that actually exists in the catalog.
fn first_present_material(
    materials: Option<&serde_json::Map<String, serde_json::Value>>,
    candidates: &[String],
) -> Option<String> {
    let m = materials?;
    candidates.iter().find(|k| m.contains_key(*k)).cloned()
}

/// Synthesize the naked base-body parts for a sex as appearance directives:
/// torso/legs/hands/feet/head + eyes/teeth, each keyed to its `eq-x-{sex}2-*-0`
/// mesh and the best available skin material. `materials` is the catalog's
/// `materials` map (used to pick a present skin per body region). Pure — no I/O.
fn base_body_directives(
    sex: char,
    materials: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Vec<AppearanceDirective> {
    let skin = |region: &str| -> Vec<String> {
        SKIN_RACE_ORDER
            .iter()
            .map(|r| format!("{r}-{sex}2-skin-{region}-1"))
            .collect()
    };
    let eyes = || -> Vec<String> {
        let mut v: Vec<String> = SKIN_RACE_ORDER
            .iter()
            .flat_map(|r| [format!("{r}-{sex}2-eyes-1"), format!("{r}-x2-eyes-1")])
            .collect();
        v.push("x-m2-eyes-1".to_string());
        v
    };
    // (mesh suffix, slot name, skin-material candidates). Slot names Chest/Legs/
    // Hands/Feet match the equipment slots so the viewer can hide a base part
    // when armor covers it; Head/Eyes are always shown (the face). Teeth are
    // deliberately excluded: the mesh is authored at the origin (not in
    // character space) so it would render on the floor — and a closed mouth
    // hides them anyway.
    let spec: [(&str, &str, Vec<String>); 6] = [
        ("chest-0", "Chest", skin("body")),
        ("legs-0", "Legs", skin("body")),
        ("hands-0", "Hands", skin("hands")),
        ("feet-0", "Feet", skin("feet")),
        ("head-0", "Head", skin("face")),
        ("eyes-0", "Eyes", eyes()),
    ];

    spec.into_iter()
        .map(|(mesh_suffix, slot, mat_candidates)| AppearanceDirective {
            slot: slot.to_string(),
            mesh_key: Some(format!("eq-x-{sex}2-{mesh_suffix}")),
            material_key: first_present_material(materials, &mat_candidates),
            dye_order: None,
            // Base skin renders with its natural texture (no player dye channels).
            dyeable: false,
            props: Default::default(),
        })
        .collect()
}

/// Resolve the naked base body for a sex into renderable slots (mesh + skin
/// material), joined against the local asset catalog. Drives the Model Viewer's
/// "Character" paper-doll mode: the body onto which equipped gear is layered.
#[tauri::command]
pub async fn resolve_base_body(
    sex: Option<String>,
    app: AppHandle,
) -> Result<Vec<ResolvedSlot>, String> {
    let sex_ch = sex
        .as_deref()
        .and_then(|s| s.chars().next())
        .filter(|c| *c == 'm' || *c == 'f')
        .unwrap_or('m');

    let cat = load_catalog(&app)?;
    let materials = cat.get("materials").and_then(|v| v.as_object());
    let slots = base_body_directives(sex_ch, materials)
        .iter()
        .map(|d| join_catalog(&cat, d))
        .collect();
    Ok(slots)
}

/// A single browsable dyeable/equippable item for the Model Viewer picker.
#[derive(Serialize)]
pub struct BrowsableItem {
    pub id: u32,
    pub name: String,
    pub icon_id: Option<u32>,
    pub equip_slot: Option<String>,
    /// True if this item's appearance resolves to a mesh present in the cache.
    /// Many cosmetics (holiday hats, masks, floating gems) have no game mesh.
    pub has_model: bool,
    pub invisible: bool,
    pub not_dyeable: bool,
    /// The raw appearance string (EquipAppearance2 preferred). Items sharing
    /// this string render pixel-identically — the UI groups the list by it
    /// (gear tiers like Amazing/Astounding/Awesome share one model).
    pub appearance_key: Option<String>,
}

/// Does any mesh key of this item's appearance exist in the extracted catalog?
fn item_has_model(item: &crate::game_data::ItemInfo, cat: &serde_json::Value) -> bool {
    let parsed = match appearance::resolve_item_appearance(
        item.equip_appearance.as_deref(),
        item.equip_appearance2.as_deref(),
        'm',
    ) {
        Some(p) => p,
        None => return false,
    };
    parsed.directives.iter().any(|d| {
        d.mesh_key.as_ref().is_some_and(|k| {
            cat.get("appearances").and_then(|s| s.get(k)).is_some()
                || cat.get("weapons").and_then(|s| s.get(k)).is_some()
        })
    })
}

/// List items that carry an equipped 3D appearance (for the viewer's item
/// picker). Optionally filter by equip slot. Each item is flagged `has_model`
/// so the UI can hide/mark ones with no extractable mesh.
#[tauri::command]
pub async fn list_appearance_items(
    slot: Option<String>,
    app: AppHandle,
    state: State<'_, GameDataState>,
) -> Result<Vec<BrowsableItem>, String> {
    // Fail open: if the catalog can't load, mark everything has_model so we
    // never hide the whole list.
    let cat = load_catalog(&app).ok();
    let data = state.read().await;
    let mut out: Vec<BrowsableItem> = data
        .items
        .values()
        .filter(|i| i.equip_appearance.is_some() || i.equip_appearance2.is_some() || i.invisible())
        .filter(|i| match &slot {
            // "OffHand" also covers shields (equip_slot "OffHandShield").
            Some(s) if s == "OffHand" => matches!(
                i.equip_slot.as_deref(),
                Some("OffHand") | Some("OffHandShield")
            ),
            Some(s) => i.equip_slot.as_deref() == Some(s.as_str()),
            None => true,
        })
        .map(|i| BrowsableItem {
            id: i.id,
            name: i.name.clone(),
            icon_id: i.icon_id,
            equip_slot: i.equip_slot.clone(),
            has_model: cat.as_ref().map_or(true, |c| item_has_model(i, c)),
            invisible: i.invisible(),
            not_dyeable: i.keywords.iter().any(|k| k == "NotDyeable"),
            appearance_key: i
                .equip_appearance2
                .clone()
                .or_else(|| i.equip_appearance.clone()),
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

// ── Extraction ────────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
struct ExtractionProgress {
    stage: String,
    message: String,
    done: bool,
    ok: bool,
}

/// Locate the extractor script. In dev this is the repo's
/// `tools/model_extractor/extract.py`; release builds ship a sidecar (TODO).
/// How to launch the extractor: a bundled frozen binary (release) or the raw
/// Python script (dev).
enum Extractor {
    /// PyInstaller-frozen sidecar shipped next to the app exe (`externalBin`).
    Sidecar(PathBuf),
    /// Dev: `python <repo>/tools/model_extractor/extract.py`.
    PythonScript(PathBuf),
}

fn resolve_extractor() -> Option<Extractor> {
    // Release: Tauri places the `externalBin` sidecar next to the main exe.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let name = if cfg!(windows) { "model-extractor.exe" } else { "model-extractor" };
            let p = dir.join(name);
            if p.is_file() {
                return Some(Extractor::Sidecar(p));
            }
        }
    }
    // Dev: the repo script, resolved from the crate manifest dir at build time.
    if let Some(manifest) = option_env!("CARGO_MANIFEST_DIR") {
        let p = Path::new(manifest)
            .parent() // repo root (manifest is src-tauri)
            .map(|r| r.join("tools").join("model_extractor").join("extract.py"));
        if let Some(p) = p {
            if p.is_file() {
                return Some(Extractor::PythonScript(p));
            }
        }
    }
    None
}

/// Run the extractor against the local install, writing into the model cache.
/// Emits `model-extraction-progress` events. Long-running.
#[derive(Debug, Serialize)]
pub struct ExtractionError {
    message: String,
    request_unity_version: bool,
}

impl From<String> for ExtractionError {
    fn from(message: String) -> Self {
        Self { message, request_unity_version: false }
    }
}

fn valid_unity_version(version: &str) -> bool {
    let parts: Vec<_> = version.split('.').collect();
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if parts.len() != 3 || !digits(parts[0]) || !digits(parts[1]) { return false; }
    let Some(index) = parts[2].find(['a', 'b', 'f', 'p']) else { return false; };
    digits(&parts[2][..index]) && digits(&parts[2][index + 1..])
}

#[tauri::command]
pub async fn start_model_extraction(
    app: AppHandle,
    game_dir: Option<String>,
    unity_version: Option<String>,
) -> Result<String, ExtractionError> {
    if unity_version.as_deref().is_some_and(|v| !valid_unity_version(v)) {
        return Err("Enter a complete Unity version, such as 6000.3.11f1.".to_string().into());
    }
    let bundles = locate_bundles_dir(game_dir.as_deref())
        .ok_or_else(|| "Could not locate a Project: Gorgon install".to_string())?;
    let out = model_cache_dir(&app)?;
    let extractor = resolve_extractor().ok_or_else(|| {
        "Model extractor not found — no bundled sidecar and no dev script".to_string()
    })?;

    let emit = |app: &AppHandle, stage: &str, message: &str, done: bool, ok: bool| {
        let _ = app.emit(
            "model-extraction-progress",
            ExtractionProgress {
                stage: stage.into(),
                message: message.into(),
                done,
                ok,
            },
        );
    };
    emit(&app, "start", &format!("Extracting from {}", bundles.display()), false, true);

    // Sidecar runs directly; the dev script runs under the system Python.
    let mut cmd = match &extractor {
        Extractor::Sidecar(bin) => tokio::process::Command::new(bin),
        Extractor::PythonScript(script) => {
            let py = if cfg!(windows) { "python" } else { "python3" };
            let mut c = tokio::process::Command::new(py);
            c.arg(script);
            c
        }
    };
    cmd.arg("--game-dir").arg(&bundles).arg("--out").arg(&out);
    if let Some(version) = &unity_version {
        cmd.arg("--unity-version").arg(version);
    }
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW — no console flash
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to launch extractor: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        emit(&app, "error", &stderr, true, false);
        return Err(ExtractionError {
            message: format!("Extractor failed (code {:?}):\n{stderr}\n{stdout}", output.status.code()),
            request_unity_version: cfg!(any(target_os = "macos", target_os = "linux")),
        });
    }
    let summary = stdout.lines().last().unwrap_or("done").to_string();
    emit(&app, "done", &summary, true, true);
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Map, Value};

    #[test]
    fn explicit_install_validation_and_bundle_path_normalization() {
        let root = std::env::temp_dir().join(format!(
            "pgmv-install-test-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let bundles = root.join(BUNDLE_SUBPATH);
        std::fs::create_dir_all(&bundles).unwrap();
        std::fs::write(bundles.join("defaultlocalgroup_assets_newmodel_assets_all_test.bundle"), b"test").unwrap();
        if cfg!(windows) {
            assert_eq!(locate_bundles_dir(root.to_str()), None);
        }
        std::fs::write(root.join("WindowsPlayer.exe"), b"test").unwrap();
        assert_eq!(locate_bundles_dir(root.to_str()), Some(bundles.clone()));
        assert_eq!(locate_bundles_dir(bundles.to_str()), Some(bundles.clone()));
        assert_eq!(locate_bundles_dir(bundles.parent().unwrap().to_str()), Some(bundles.clone()));
        let quoted = format!("  \"{}\"  ", root.display());
        assert_eq!(locate_bundles_dir(Some(&quoted)), Some(bundles));
        // Invalid explicit input must not silently resolve to the real Steam install.
        assert_eq!(locate_bundles_dir(root.join("missing").to_str()), None);
        std::fs::remove_dir_all(&root).unwrap();
    }

    fn mats(keys: &[&str]) -> Map<String, Value> {
        keys.iter().map(|k| ((*k).to_string(), json!({}))).collect()
    }

    #[test]
    fn native_install_bundle_layouts() {
        for subpath in [
            "ProjectGorgon_Data/StreamingAssets/aa/StandaloneLinux64",
            "MacPlayer.app/Contents/Resources/Data/StreamingAssets/aa/StandaloneOSX",
            "MacPlayer.app/Contents/Data/StreamingAssets/aa/StandaloneOSX",
        ] {
            let root = std::env::temp_dir().join(format!("pgmv-native-test-{}-{}",
                std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
            let bundles = root.join(subpath);
            std::fs::create_dir_all(&bundles).unwrap();
            // An arbitrary directory alone must not count as a game install.
            assert_eq!(native_bundles_in_install(&root), None);
            std::fs::write(bundles.join("defaultlocalgroup_assets_newmodel_assets_all_test.bundle"), b"test").unwrap();
            assert_eq!(native_bundles_in_install(&root), Some(bundles.clone()));
            assert_eq!(native_bundles_in_install(&bundles), Some(bundles.clone()));
            assert_eq!(native_bundles_in_install(bundles.parent().unwrap()), Some(bundles.clone()));
            if subpath.starts_with("MacPlayer.app") {
                assert_eq!(native_bundles_in_install(&root.join("MacPlayer.app")), Some(bundles));
            }
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn unity_version_validation_requires_full_version() {
        for version in ["6000.3.11f1", "2022.3.0b2", "6000.1.1a1", "2021.3.2p1"] {
            assert!(valid_unity_version(version));
        }
        for version in ["", "6000.3.11", "6000.3.11f", "6000.3.11f1extra", "6000.3.f1", "--help", "6000.3.11f1\n"] {
            assert!(!valid_unity_version(version));
        }
    }

    #[test]
    fn base_body_uses_expected_meshes_and_slots() {
        let m = mats(&["f-m2-skin-body-1", "f-m2-skin-hands-1"]);
        let d = base_body_directives('m', Some(&m));
        let slots: Vec<&str> = d.iter().map(|x| x.slot.as_str()).collect();
        // No Teeth: that mesh is origin-authored and would render on the floor.
        assert_eq!(slots, ["Chest", "Legs", "Hands", "Feet", "Head", "Eyes"]);
        // Meshes are the sex-specific naked base parts.
        assert_eq!(
            d[0].mesh_key.as_deref(),
            Some("eq-x-m2-chest-0"),
            "chest mesh key"
        );
        assert_eq!(d[3].mesh_key.as_deref(), Some("eq-x-m2-feet-0"));
        // Never player-dyeable — base skin renders with its own texture.
        assert!(d.iter().all(|x| !x.dyeable));
    }

    #[test]
    fn skin_material_prefers_present_race_and_falls_back() {
        // `f` skin-body present → picked first (top of SKIN_RACE_ORDER).
        let m = mats(&["f-m2-skin-body-1", "x-m2-skin-body-1"]);
        let d = base_body_directives('m', Some(&m));
        assert_eq!(d[0].material_key.as_deref(), Some("f-m2-skin-body-1"));

        // No `f` body skin → fall through to the next present race (`x`).
        let m2 = mats(&["x-m2-skin-body-1"]);
        let d2 = base_body_directives('m', Some(&m2));
        assert_eq!(d2[0].material_key.as_deref(), Some("x-m2-skin-body-1"));
    }

    #[test]
    fn eyes_fall_back_across_sexes() {
        // Only an `x-m2-eyes-1` exists — eyes candidates must still find it for male.
        let m = mats(&["x-m2-eyes-1"]);
        let d = base_body_directives('m', Some(&m));
        let eyes = d.iter().find(|x| x.slot == "Eyes").unwrap();
        assert_eq!(eyes.material_key.as_deref(), Some("x-m2-eyes-1"));
    }

    #[test]
    fn female_meshes_use_f2_keys() {
        let d = base_body_directives('f', None);
        assert_eq!(d[0].mesh_key.as_deref(), Some("eq-x-f2-chest-0"));
        assert_eq!(d[4].mesh_key.as_deref(), Some("eq-x-f2-head-0"));
        // No catalog → no materials matched, but directives still enumerate.
        assert!(d.iter().all(|x| x.material_key.is_none()));
    }
}
