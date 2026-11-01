<template>
  <div class="relative w-full h-full min-h-0">
    <div ref="container" class="absolute inset-0"></div>

    <!-- Overlay controls -->
    <div class="absolute top-2 right-2 flex gap-2">
      <button
        class="px-2 py-1 text-xs rounded border border-white/15 bg-black/40 text-white/80 hover:bg-black/60 cursor-pointer"
        :aria-pressed="autoSpin"
        @click="autoSpin = !autoSpin">
        {{ autoSpin ? "⏸ Spin" : "▶ Spin" }}
      </button>
      <button
        class="px-2 py-1 text-xs rounded border border-white/15 bg-black/40 text-white/80 hover:bg-black/60 cursor-pointer"
        @click="resetView">
        ⟲ Reset
      </button>
    </div>

    <div
      v-if="empty"
      class="absolute inset-0 flex items-center justify-center text-sm text-text-muted pointer-events-none">
      {{ emptyMessage }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, watch, computed } from "vue";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { RoomEnvironment } from "three/addons/environments/RoomEnvironment.js";
import { useModelViewerStore } from "../../../stores/modelViewerStore";
import { useSettingsStore } from "../../../stores/settingsStore";
import {
  createDyeMaterial,
  type DyeMaterial,
  type DyeInit,
} from "../../../composables/useGorgonDyeMaterial";
import type { ResolvedSlot } from "../../../types/modelViewer";

const store = useModelViewerStore();
const container = ref<HTMLDivElement | null>(null);
const settings = useSettingsStore();
const autoSpin = computed({
  get: () => settings.autoSpin,
  set: (enabled: boolean) => settings.setAutoSpin(enabled),
});

let renderer: THREE.WebGLRenderer | null = null;
let scene: THREE.Scene | null = null;
let camera: THREE.PerspectiveCamera | null = null;
let controls: OrbitControls | null = null;
let raf = 0;
let resizeObs: ResizeObserver | null = null;

const modelGroup = new THREE.Group();
const dyeMaterials = new Map<string, DyeMaterial>();
const gltfLoader = new GLTFLoader();
const texLoader = new THREE.TextureLoader();
let homeTarget = new THREE.Vector3();
let homeDistance = 3;

const empty = computed(() =>
  store.viewMode === "character"
    ? store.baseBody.length === 0
    : !store.resolved?.renderable,
);
const emptyMessage = computed(() => {
  if (store.viewMode === "character") {
    return store.baseBody.length === 0
      ? "Base body not available — re-extract models"
      : "";
  }
  return store.resolving
    ? "Loading model…"
    : store.resolved && !store.resolved.renderable
      ? "No 3D model available for this item"
      : "Select an item to preview";
});

function initThree() {
  const el = container.value!;
  scene = new THREE.Scene();
  scene.background = new THREE.Color(0x1a1c20);
  scene.add(modelGroup);

  camera = new THREE.PerspectiveCamera(35, el.clientWidth / el.clientHeight, 0.05, 100);
  camera.position.set(0, 1, 3);

  renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.setSize(el.clientWidth, el.clientHeight);
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.0;
  el.appendChild(renderer.domElement);

  // Soft studio environment for PBR reflections.
  const pmrem = new THREE.PMREMGenerator(renderer);
  scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;

  const key = new THREE.DirectionalLight(0xffffff, 2.0);
  key.position.set(2, 3, 2);
  scene.add(key);
  const fill = new THREE.DirectionalLight(0xbcd2ff, 0.7);
  fill.position.set(-2, 1, -1);
  scene.add(fill);
  scene.add(new THREE.HemisphereLight(0xffffff, 0x2a2a33, 0.5));

  controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;
  controls.dampingFactor = 0.08;
  controls.target.set(0, 1, 0);
  // Turntable feel: horizontal drag orbits around the vertical (waist) axis;
  // clamp the vertical angle so the model can never tumble/cartwheel over the
  // top or flip to show its underside.
  controls.minPolarAngle = Math.PI * 0.3; // ~54° — don't rise to a full top-down
  controls.maxPolarAngle = Math.PI * 0.5; // 90° — don't drop below eye level

  resizeObs = new ResizeObserver(onResize);
  resizeObs.observe(el);

  animate();
  void rebuild();
}

function onResize() {
  if (!renderer || !camera || !container.value) return;
  const el = container.value;
  camera.aspect = el.clientWidth / el.clientHeight;
  camera.updateProjectionMatrix();
  renderer.setSize(el.clientWidth, el.clientHeight);
}

function animate() {
  raf = requestAnimationFrame(animate);
  if (autoSpin.value) modelGroup.rotation.y += 0.006;
  controls?.update();
  if (renderer && scene && camera) renderer.render(scene, camera);
}

function clearModel() {
  modelGroup.rotation.set(0, 0, 0);
  for (const child of [...modelGroup.children]) {
    modelGroup.remove(child);
    child.traverse((o) => {
      const mesh = o as THREE.Mesh;
      if (mesh.geometry) mesh.geometry.dispose();
    });
  }
  for (const dm of dyeMaterials.values()) dm.dispose();
  dyeMaterials.clear();
}

function loadTexture(rel: string | undefined | null): THREE.Texture | null {
  if (!rel) return null;
  const t = texLoader.load(store.assetUrl(`textures/${rel}`));
  t.anisotropy = 8;
  t.flipY = false; // glTF/textures already in correct orientation
  // Mirrored gear parts (pauldrons, skirt panels) tile UVs past 1 and rely on
  // wrap — three.js defaults to clamp-to-edge, which renders them as a flat
  // smear of the edge texel (e.g. the Umrad Coat's black left pauldron).
  t.wrapS = THREE.RepeatWrapping;
  t.wrapT = THREE.RepeatWrapping;
  return t;
}

function hexToRgb(hex: string): [number, number, number] | undefined {
  const m = /^#?([0-9a-fA-F]{6})$/.exec(hex.trim());
  if (!m) return undefined;
  const n = parseInt(m[1], 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

function dyeInitFor(slot: ResolvedSlot): DyeInit {
  const c = slot.default_colors ?? {};
  const rgb = (k: string): [number, number, number] | undefined => {
    const v = c[k];
    return v ? [v[0], v[1], v[2]] : undefined;
  };
  // Prefer any user-set dye stored in the store (persists across rebuilds),
  // falling back to the material's default color for the channel.
  const stored = store.getSlotDye(slot.slot);
  const ch = (n: number): [number, number, number] | undefined =>
    (stored?.[n] && hexToRgb(stored[n])) || rgb(`_Color${n}`);
  return {
    channels: slot.dyeable ? slot.dye_channels : 0,
    color0: rgb("_Color0"),
    color1: ch(1),
    color2: ch(2),
    color3: ch(3),
  };
}

/** Placement of a held weapon on the paper doll (Character mode). */
interface HandPlacement {
  /** Group-space palm position where the weapon's grip (mesh origin) goes. */
  anchor: THREE.Vector3;
  /** World-X direction away from the body: +1 = character's left, −1 = right. */
  outward: 1 | -1;
  /** OffHandShield directive — the plate faces outward from the forearm. */
  isShield: boolean;
}

/** Place a held weapon at the hand. Weapons are authored Y-up with the grip at
 * the mesh origin (verified: sword blade spans −0.17..+0.84 Y, staff/knife/
 * hammer similar, bow gripped at center), so no reorientation is needed —
 * except shields (an XY plate, origin at the back face where the forearm sits,
 * facing +Z → turned to face away from the body) and Z-long oddballs like
 * instruments (stood upright). */
function placeWeaponInHand(mesh: THREE.Mesh, hand: HandPlacement) {
  mesh.geometry.computeBoundingBox();
  const size = new THREE.Vector3();
  mesh.geometry.boundingBox!.getSize(size);
  if (hand.isShield) {
    mesh.rotation.y = (Math.PI / 2) * hand.outward;
  } else if (size.z > size.y && size.z > size.x) {
    mesh.rotation.x = -Math.PI / 2;
  }
  mesh.position.copy(hand.anchor);
}

/** Stand a weapon/prop upright: longest bbox axis → Y (up), thinnest → Z
 * (depth), so shields face the camera and staves/swords stand on end. */
function orientProp(mesh: THREE.Mesh) {
  mesh.geometry.computeBoundingBox();
  const size = new THREE.Vector3();
  mesh.geometry.boundingBox!.getSize(size);
  const axisVec: Record<string, THREE.Vector3> = {
    x: new THREE.Vector3(1, 0, 0),
    y: new THREE.Vector3(0, 1, 0),
    z: new THREE.Vector3(0, 0, 1),
  };
  const ordered = (["x", "y", "z"] as const)
    .map((a) => [a, size[a]] as const)
    .sort((a, b) => b[1] - a[1])
    .map(([a]) => a);
  const long = axisVec[ordered[0]];
  const mid = axisVec[ordered[1]];
  const short = axisVec[ordered[2]].clone();
  // R⁻¹ columns = [mid, long, short] ⇒ R maps long→Y, mid→X, short→Z.
  const rInv = new THREE.Matrix4().makeBasis(mid, long, short);
  if (rInv.determinant() < 0) {
    short.negate();
    rInv.makeBasis(mid, long, short);
  }
  mesh.quaternion.setFromRotationMatrix(rInv.invert());
}

/**
 * Load one resolved slot's geometry, apply its Gorgon/Character dye material,
 * and add it to the model group. Body meshes carry their world offset in the
 * vertices (bind-pose character space) so multiple pieces auto-assemble into a
 * standing figure; weapons are oriented by bounding box instead — unless a
 * `hand` placement is given (Character mode), which grips them at the palm.
 * `dyeKey` names the material in `dyeMaterials` for live recoloring (defaults
 * to the slot). Returns the added mesh (or null if skipped/superseded).
 */
async function addMesh(
  slot: ResolvedSlot,
  seq: number,
  dyeKey = slot.slot,
  hand?: HandPlacement,
): Promise<THREE.Mesh | null> {
  if (!slot.mesh_file) return null;
  const gltf = await gltfLoader.loadAsync(store.assetUrl(slot.mesh_file));
  // A newer rebuild may have started (and cleared the scene) while this mesh
  // was loading — if so, drop it instead of appending into the fresh scene.
  if (seq !== buildSeq) return null;
  let found: THREE.Mesh | null = null;
  gltf.scene.traverse((o) => {
    if ((o as THREE.Mesh).isMesh && !found) found = o as THREE.Mesh;
  });
  if (!found) return null;
  const mesh: THREE.Mesh = found;

  if (slot.is_weapon && hand) {
    placeWeaponInHand(mesh, hand);
  } else if (slot.is_weapon) {
    // Weapons/props have varied native orientations. Stand them up generically:
    // longest bounding-box axis → vertical (Y), thinnest → depth (Z, toward the
    // camera) so flat items like shields face front.
    orientProp(mesh);
  } else {
    // Project: Gorgon character art is authored Z-up; three.js is Y-up. Rotate
    // each body mesh so the character stands upright — then the turntable spins
    // around its true vertical (spine/waist) axis instead of tumbling.
    mesh.rotation.x = -Math.PI / 2;
  }

  const tex = slot.textures ?? {};
  const dm = createDyeMaterial(
    {
      skin: loadTexture(tex.skin),
      mask1: loadTexture(tex.mask1),
      mask2: loadTexture(tex.mask2),
      mask3: loadTexture(tex.mask3),
      normal: loadTexture(tex.normal),
    },
    dyeInitFor(slot),
  );
  mesh.material = dm.material;
  dyeMaterials.set(dyeKey, dm);
  modelGroup.add(mesh);
  return mesh;
}

// Measured palm centroids of the naked base hands, char space (Z-up), used
// only if no hands-region mesh loaded: (±x, y, z_height).
const PALM_FALLBACK = { m: [0.564, -0.048, 1.031], f: [0.529, -0.134, 1.026] } as const;

/**
 * Group-space palm anchors for held weapons, one per hand. Computed from the
 * hands-region mesh actually on the doll (naked hands or gloves — handles both
 * sexes and gauntlet offsets): the per-side vertex centroid ≈ the palm. The
 * character faces −Y in char space, so their right (main) hand is x<0, which
 * the −π/2 X body rotation maps to world −X (screen-left, mirrored like a
 * person facing you).
 */
function handAnchors(handsMesh: THREE.Mesh | null): { main: THREE.Vector3; off: THREE.Vector3 } {
  if (!handsMesh) {
    const [x, y, z] = PALM_FALLBACK[store.sex];
    // char (x,y,z) → group (x, z, −y) under the −π/2 X rotation.
    return { main: new THREE.Vector3(-x, z, -y), off: new THREE.Vector3(x, z, -y) };
  }
  const pos = handsMesh.geometry.getAttribute("position");
  // Gauntlets extend up the forearm and would drag a whole-mesh centroid
  // toward the elbow; the physical hand is always the bottom of the region
  // (mesh-local char space is Z-up), so average only the lowest 15cm per side.
  let zMinMain = Infinity;
  let zMinOff = Infinity;
  for (let i = 0; i < pos.count; i++) {
    const z = pos.getZ(i);
    if (pos.getX(i) < 0) zMinMain = Math.min(zMinMain, z);
    else zMinOff = Math.min(zMinOff, z);
  }
  const main = new THREE.Vector3();
  const off = new THREE.Vector3();
  let nMain = 0;
  let nOff = 0;
  for (let i = 0; i < pos.count; i++) {
    const x = pos.getX(i);
    const z = pos.getZ(i);
    if (z > (x < 0 ? zMinMain : zMinOff) + 0.15) continue;
    const v = x < 0 ? main : off;
    v.x += x;
    v.y += pos.getY(i);
    v.z += z;
    if (x < 0) nMain++;
    else nOff++;
  }
  if (nMain > 0) main.divideScalar(nMain);
  if (nOff > 0) off.divideScalar(nOff);
  // Mesh-local (char space) → group space via the mesh's own transform
  // (position is still 0 here — frameCamera recenters only after the build).
  handsMesh.updateMatrix();
  main.applyMatrix4(handsMesh.matrix);
  off.applyMatrix4(handsMesh.matrix);
  return { main, off };
}

// Base-body regions covered (replaced) by equipped armor in the same slot —
// hidden so the naked part doesn't poke through the gear that supersedes it.
// Head/Eyes/Teeth (the face) are always shown.
const HIDE_BASE_WHEN_EQUIPPED = new Set(["Chest", "Legs", "Hands", "Feet"]);

// Monotonic build token: a rebuild that started before a newer one aborts its
// remaining appends so overlapping async loads never double-populate the scene.
let buildSeq = 0;

async function buildItem(seq: number): Promise<void> {
  const res = store.resolved;
  if (!res || !res.renderable) return;
  for (const slot of res.slots) {
    if (!slot.mesh_file) continue;
    try {
      await addMesh(slot, seq);
    } catch (e) {
      console.warn(`Failed to load slot ${slot.slot}:`, e);
    }
    if (seq !== buildSeq) return;
  }
}

async function buildCharacter(seq: number): Promise<void> {
  // Which equipment slots have a renderable item on the doll right now.
  const equipped = store.resolvedLoadout;
  const covered = new Set(
    Object.entries(equipped)
      .filter(([, a]) => a.renderable)
      .map(([slot]) => slot),
  );

  // The hands-region mesh on the doll (naked hands or gloves) anchors held
  // weapons at the palms.
  let handsMesh: THREE.Mesh | null = null;

  // Naked base body first (skip a region that armor fully replaces).
  for (const part of store.baseBody) {
    if (HIDE_BASE_WHEN_EQUIPPED.has(part.slot) && covered.has(part.slot)) continue;
    try {
      const m = await addMesh(part, seq, `base:${part.slot}`);
      if (part.slot === "Hands" && m) handsMesh = m;
    } catch (e) {
      console.warn(`Failed to load base part ${part.slot}:`, e);
    }
    if (seq !== buildSeq) return;
  }

  // Equipped gear layered on top (weapons handled separately below — they need
  // hand placement, not the body rotation). Keyed by directive slot (same as
  // item mode) so DyeControls, which emits the active item's directive slot,
  // recolors the matching piece on the body.
  for (const [equipSlot, appr] of Object.entries(equipped)) {
    if (!appr.renderable) continue;
    for (const rs of appr.slots) {
      if (!rs.mesh_file || rs.is_weapon) continue;
      try {
        const m = await addMesh(rs, seq);
        if (rs.slot === "Hands" && m) handsMesh = m;
      } catch (e) {
        console.warn(`Failed to load ${equipSlot}/${rs.slot}:`, e);
      }
      if (seq !== buildSeq) return;
    }
  }

  // Held weapons: MainHand in the character's right hand, OffHand in the left,
  // gripped at the palm (weapon meshes carry their grip at the origin).
  const anchors = handAnchors(handsMesh);
  for (const [equipSlot, appr] of Object.entries(equipped)) {
    if (!appr.renderable) continue;
    const outward: 1 | -1 = equipSlot === "OffHand" ? 1 : -1;
    const anchor = outward === 1 ? anchors.off : anchors.main;
    for (const rs of appr.slots) {
      if (!rs.mesh_file || !rs.is_weapon) continue;
      try {
        await addMesh(rs, seq, rs.slot, {
          anchor,
          outward,
          isShield: /shield/i.test(rs.slot),
        });
      } catch (e) {
        console.warn(`Failed to load weapon ${equipSlot}/${rs.slot}:`, e);
      }
      if (seq !== buildSeq) return;
    }
  }
}

async function rebuild() {
  if (!scene) return;
  const seq = ++buildSeq;
  clearModel();
  if (store.viewMode === "character") await buildCharacter(seq);
  else await buildItem(seq);
  if (seq !== buildSeq) return; // superseded — a newer rebuild owns the scene
  frameCamera();
}

function frameCamera() {
  if (!camera || !controls || modelGroup.children.length === 0) return;
  // modelGroup.rotation is reset to 0 by clearModel(), so local == world here.
  const box = new THREE.Box3().setFromObject(modelGroup);
  if (box.isEmpty()) return;
  const size = box.getSize(new THREE.Vector3());
  const center = box.getCenter(new THREE.Vector3());
  // Recenter meshes on the group origin so the turntable spins in place.
  for (const child of modelGroup.children) child.position.sub(center);
  const maxDim = Math.max(size.x, size.y, size.z);
  const fov = (camera.fov * Math.PI) / 180;
  homeDistance = (maxDim / 2 / Math.tan(fov / 2)) * 1.5;
  homeTarget = new THREE.Vector3(0, 0, 0);
  resetView();
}

function resetView() {
  if (!camera || !controls) return;
  controls.target.copy(homeTarget);
  camera.position.set(
    homeTarget.x,
    homeTarget.y + homeDistance * 0.15,
    homeTarget.z + homeDistance,
  );
  camera.updateProjectionMatrix();
  controls.update();
}

/** Called by parent to recolor a dye channel of a slot live. */
function setDye(slot: string, channel: number, hex: string) {
  dyeMaterials.get(slot)?.setChannelColor(channel, hex);
}

defineExpose({ setDye });

// Rebuild when the active item changes (item mode), the mode toggles, or the
// paper-doll inputs change (base body / equipped loadout, incl. dye-less
// structural edits). Dye recolors go through setDye and don't rebuild.
watch(
  () => store.dyeBySlot,
  (slots) => {
    for (const [slot, colors] of Object.entries(slots)) {
      for (const [channel, hex] of Object.entries(colors)) setDye(slot, Number(channel), hex);
    }
  },
  { deep: true },
);
watch(
  () => store.resolved,
  () => {
    if (store.viewMode === "item") void rebuild();
  },
);
watch(
  () => store.viewMode,
  () => void rebuild(),
);
watch(
  [() => store.baseBody, () => store.resolvedLoadout],
  () => {
    if (store.viewMode === "character") void rebuild();
  },
  { deep: true },
);

onMounted(initThree);
onBeforeUnmount(() => {
  cancelAnimationFrame(raf);
  resizeObs?.disconnect();
  clearModel();
  controls?.dispose();
  renderer?.dispose();
  renderer?.domElement.remove();
});
</script>
