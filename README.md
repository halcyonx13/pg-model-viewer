# PG Model Viewer

A standalone desktop app that renders **real Project: Gorgon item models** with
the game's live **1/2/3-channel dye** system, plus a **paper-doll** character
mode that assembles a full loadout onto a base body.

Extracted from [glogger](https://github.com/crisp-oddio/glogger-oddio) — this is
the 3D Model Viewer feature as its own app.

## What it does

- Browse equippable items by slot (Head / Chest / Legs / Hands / Feet / Main /
  Off-hand) and view their actual in-game 3D model on a turntable.
- Recolor each piece live with Project: Gorgon's `Gorgon/Character` dye shader
  (single / double / triple dye), replicated in three.js.
- **Character mode**: dress a naked base body slot-by-slot and see the whole
  loadout assembled and dyed.

## How it works

```
PG CDN items.json  ──▶  item → EquipAppearance2 directive  (game_data/appearance.rs)
                          │  @Chest=@eq-{sex}2-chest-leather-01(^Armor=…%DYE%)
                          ▼
local Unity bundles  ──▶  extractor (tools/model_extractor/extract.py)
  (your PG install)        → {appdata}/models/  meshes/*.glb + textures/*.png + catalog.json
                          ▼
three.js viewer      ──▶  mesh geometry + dye ShaderMaterial (live recolor via uniforms)
```

- **Item data** comes from the public PG CDN (`items.json`), fetched and cached
  on first launch — no game files needed for browsing.
- **3D geometry + textures** are extracted from **your own local Project: Gorgon
  install** (unencrypted Unity 6.3 Addressable bundles) into a local cache. No
  game art is redistributed.

## Requirements

- Node 18+, Rust (stable), and the Tauri 2 prerequisites for your OS.
- **Python 3** with `UnityPy`, `pygltflib`, `numpy`, `Pillow`
  (`pip install -r tools/model_extractor/requirements.txt`) for the one-time
  model extraction — **dev only**; release builds ship a frozen extractor.
- A local Project: Gorgon install (auto-detected via Steam).

## Develop

```bash
npm install
npm run tauri dev      # full app (Vite + Tauri)
# or, frontend only:
npm run dev
```

Backend checks / tests:

```bash
cd src-tauri
cargo test --lib        # includes the appearance parser + base-body unit tests
```

## Install (users)

Grab the installer for your platform from the
[latest release](https://github.com/crisp-oddio/pg-model-viewer/releases/latest):
Windows `-setup.exe`, macOS `.dmg` (Apple Silicon), Linux `.deb` / `.AppImage`.
Release builds bundle the model extractor — no Python needed.

## Release (maintainers)

Actions → **Release** → Run workflow → choose `patch`/`minor`/`major` or an
explicit `x.y.z`. The workflow bumps the version, tags, freezes the extractor
sidecar per platform, builds all installers, and publishes the GitHub Release.

## First run

1. Launch the app. It fetches `items.json` and shows the item picker.
2. Click **Extract models** — a one-time (~1 min) extraction from your local
   install into `{appdata}/models/`.
3. Pick a slot + item to view it; toggle **Character** for the paper doll.

On Windows the app first checks
`C:\Program Files (x86)\Steam\steamapps\common\Project Gorgon`, then other
Steam libraries. If it cannot find the game, enter your install folder in the
app. It validates `WindowsPlayer.exe` and the asset bundle directory, remembers
the location, and uses it when launching the bundled extractor. The extractor
reads that install's Unity version automatically.

> **Note:** the extraction spawns `python` on your PATH. The Microsoft-Store
> Python is sandboxed and cannot write `%APPDATA%\Roaming` — if extraction
> fails, use a non-Store Python (python.org) or pre-seed the cache.

## Known limitations / follow-ups

### Dye profiles

Open **Dye Profiles** beside **Settings** to show the creator in the middle pane.
Choose three dyes from the named color
grid, and save them with a profile name. Click a numbered position to replace
its dye. Profiles are saved locally and remain available after restarting.
Use **Apply dye profile…** above a piece's dye channels to apply the first one,
two, or three colors to match its channel count. In Character mode, apply a
profile separately to each piece. Drag a channel's handle onto another channel
to move its dye into that position; the arrow buttons perform the same action.

Click a saved profile to load its name and dyes into the editor. **Save changes**
overwrites that profile; changing its name offers **Save new** instead. The three
selected dyes can also be reordered by dragging or using the left/right arrows.
Use **Back to model** (or the header button again) to return to the viewer;
the model, camera, and unfinished profile edits are preserved. Main Hand and
Offhand selections hide all equipment dye controls in the right-hand pane.
Choose **Set default** beside a saved profile, then enable **Automatically apply
default dye profile** to color current dyeable equipment and newly chosen pieces.
The default and checkbox preference persist across restarts. Disabling automatic
application leaves already applied colors in place. Saved loadouts keep their
saved dye choices when restored.

The extractor automatically reads the Unity Product version from
`WindowsPlayer.exe` on Windows for bundles with missing version metadata. If
unavailable, it logs and uses the last verified version (`6000.3.11f1`). When
running the Python extractor manually, use `--unity-version 6000.3.11f1` to
override detection. Bundles with valid version metadata retain their own version.

On macOS and Linux, extraction initially uses the bundles' version metadata.
If the extractor exits with an error, the app shows its diagnostics and prompts
for the installed client's Unity engine version (including a suffix such as
`f1`). Enter the version and click **Retry extraction**; the app passes it to
the extractor with `--unity-version`. A failed retry keeps the prompt available
for correction, or you can cancel. Startup/installation errors do not trigger
this prompt. Native Steam install paths, `.app` bundles, and platform asset
directories are supported by discovery; these paths and the retry flow have
automated coverage but still need testing on actual macOS and Linux machines.

- **Weapons in Character mode** aren't placed in-hand yet (item-mode turntable
  shows them fine).
- **Loadouts aren't persisted** yet, and dye edits reset on rebuild — both land
  with a saved-preset feature.
- Base body is a static bind/A-pose (no skeleton/animation); hair and appearance
  flags (`Hair=Off`, `Bra=off`, …) aren't rendered.
- Drop-source data (which monster drops an item) was glogger-specific and is not
  included here.

## License

GPL-3.0, following glogger.
