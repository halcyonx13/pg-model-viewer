<template>
  <div class="flex flex-col gap-3 h-full overflow-hidden">
    <!-- Loading -->
    <div v-if="store.loading" class="flex items-center justify-center h-full text-text-muted">
      Loading model viewer…
    </div>

    <!-- Game install not found -->
    <div
      v-else-if="!store.gameFound"
      class="flex flex-col items-center justify-center h-full gap-2 text-center px-6">
      <div class="text-lg font-medium text-text-secondary">Project: Gorgon install not found</div>
      <p class="text-sm text-text-muted max-w-md">
        The Model Viewer extracts 3D models from your local game install. We
        couldn't find it in the default Steam folder or your Steam libraries.
        Enter the Project Gorgon install folder containing WindowsPlayer.exe.
      </p>
      <form class="flex flex-col gap-2 w-full max-w-lg" @submit.prevent="store.useGameDirectory(installFolder)">
        <label for="game-install-folder" class="text-sm text-text-secondary text-left">Project Gorgon install folder</label>
        <input
          id="game-install-folder"
          v-model="installFolder"
          type="text"
          class="input text-sm w-full"
          placeholder="C:\Program Files (x86)\Steam\steamapps\common\Project Gorgon"
          :disabled="store.checkingInstall"
          autocomplete="off" />
        <button class="btn-primary" type="submit" :disabled="store.checkingInstall || !installFolder.trim()">
          {{ store.checkingInstall ? "Checking…" : "Use this folder" }}
        </button>
      </form>
      <div v-if="store.error" role="alert" class="text-xs text-accent-red max-w-lg break-words">{{ store.error }}</div>
      <button class="btn-secondary mt-2" @click="store.refreshStatus()">Re-check</button>
    </div>

    <!-- Cache not built yet -->
    <div
      v-else-if="!store.cacheReady"
      class="flex flex-col items-center justify-center h-full gap-3 text-center px-6">
      <div class="text-lg font-medium text-text-secondary">Extract 3D models</div>
      <p class="text-sm text-text-muted max-w-md">
        One-time extraction of gear models and textures from your local
        Project: Gorgon install into the app's cache. This takes about a minute.
      </p>
      <button class="btn-primary" :disabled="store.extracting" @click="store.runExtraction()">
        {{ store.extracting ? "Extracting…" : "Extract models" }}
      </button>
      <div v-if="store.extracting" class="text-xs text-text-muted">
        {{ store.extractionMessage }}
      </div>
      <div v-if="store.error" class="text-xs text-accent-red max-w-md break-words">{{ store.error }}</div>
    </div>

    <!-- Main viewer -->
    <PaneLayout
      v-else
      screen-key="model-viewer"
      :left-pane="{ title: 'Items', fixedWidth: 300 }"
      :right-pane="{ title: 'Customize', defaultWidth: 320, minWidth: 240, maxWidth: 520 }">
      <!-- Left: items for the active slot -->
      <template #left>
        <div class="flex flex-col gap-2 h-full min-h-0 p-2">
          <!-- Sex toggle -->
          <div class="flex gap-1">
            <button
              v-for="opt in [{ v: 'm', l: 'Male' }, { v: 'f', l: 'Female' }]"
              :key="opt.v"
              class="flex-1 px-2 py-1 text-xs rounded border cursor-pointer"
              :class="store.sex === opt.v
                ? 'bg-accent-gold/20 border-accent-gold/40 text-accent-gold'
                : 'border-white/10 text-text-muted hover:bg-white/5'"
              @click="store.setSex(opt.v as 'm' | 'f')">
              {{ opt.l }}
            </button>
          </div>

          <div class="flex items-center justify-between">
            <span class="text-[11px] text-text-dim uppercase tracking-wide">
              {{ activeSlotLabel }} items
            </span>
            <span class="text-[10px] text-text-dim">
              {{ groupModels ? `${matchedGroups.length} models` : `${modeledCount} with model` }}
            </span>
          </div>

          <label class="flex items-center gap-1.5 text-[11px] text-text-muted cursor-pointer select-none">
            <input v-model="onlyInvisible" type="checkbox" class="cursor-pointer" />
            Only invisible items
          </label>
          <label class="flex items-center gap-1.5 text-[11px] text-text-muted cursor-pointer select-none">
            <input v-model="groupModels" type="checkbox" class="cursor-pointer" />
            Group items sharing a model
          </label>

          <!-- Search -->
          <input
            v-model="search"
            type="text"
            :placeholder="`Search ${activeSlotLabel}…`"
            class="input text-xs" />

          <!-- Item list (windowed: renders in chunks, grows on scroll) -->
          <div
            ref="listEl"
            class="flex-1 min-h-0 overflow-y-auto flex flex-col gap-0.5"
            @scroll="onListScroll">
            <button
              v-for="g in displayedGroups"
              :key="g.rep.id"
              class="flex items-center gap-2 px-2 py-1 rounded text-left text-xs cursor-pointer hover:bg-white/5"
              :class="[
                { 'bg-accent-gold/15': isGroupChosen(g) },
                g.rep.has_model || onlyInvisible ? '' : 'opacity-40',
              ]"
              :title="groupTitle(g)"
              @click="store.chooseItem(g.rep)">
              <GameIcon :icon-id="g.rep.icon_id ?? undefined" size="sm" />
              <span class="truncate">{{ g.rep.name }}</span>
              <span
                v-if="g.ids.length > 1"
                class="ml-auto shrink-0 text-[9px] px-1 rounded bg-white/10 text-text-muted"
                >×{{ g.ids.length }}</span
              >
              <span
                v-if="!g.rep.has_model"
                class="shrink-0 text-[9px] text-text-dim"
                :class="{ 'ml-auto': g.ids.length <= 1 }"
                >no model</span
              >
            </button>
            <div v-if="store.itemsLoading" class="text-xs text-text-muted px-2 py-1">Loading…</div>
            <div
              v-else-if="matchedGroups.length === 0"
              class="text-xs text-text-muted px-2 py-1">
              No {{ activeSlotLabel.toLowerCase() }} items.
            </div>
            <div
              v-else-if="displayedGroups.length < matchedGroups.length"
              class="text-[10px] text-text-dim px-2 py-1">
              Showing {{ displayedGroups.length }} of {{ matchedGroups.length }} — scroll for more
            </div>
          </div>
        </div>
      </template>

      <!-- Center: slot column + viewport (Item / Character) -->
      <div class="relative flex flex-col h-full min-h-0">
        <!-- Item / Character toggle (top of the model pane) -->
        <div class="flex gap-1 p-2">
          <button
            v-for="m in ([['item', 'Item'], ['character', 'Character']] as const)"
            :key="m[0]"
            class="px-3 py-1 text-xs rounded border cursor-pointer"
            :class="store.viewMode === m[0]
              ? 'bg-accent-gold/20 border-accent-gold/40 text-accent-gold'
              : 'border-white/10 text-text-muted hover:bg-white/5'"
            @click="store.setViewMode(m[0])">
            {{ m[1] }}
          </button>
        </div>

        <!-- Slot column (nested on the left, dropped down to align with the
             item list) + viewport -->
        <div class="flex flex-1 min-h-0">
          <div class="flex flex-col gap-1.5 pl-2 pr-1 pt-12 shrink-0">
            <button
              v-for="s in VIEWER_SLOTS"
              :key="s.id"
              class="relative w-[60px] h-[60px] p-0.5 rounded border flex items-center justify-center cursor-pointer transition-colors"
              :class="store.activeSlot === s.id
                ? 'border-accent-gold bg-accent-gold/10'
                : 'border-border-default hover:bg-white/5'"
              :title="s.label"
              @click="store.selectSlot(s.id)">
              <GameIcon
                v-if="store.loadout[s.id]"
                :icon-id="store.loadout[s.id].icon_id ?? undefined"
                size="fill" />
              <span v-else class="text-[9px] text-text-dim leading-tight text-center px-0.5">
                {{ s.label }}
              </span>
              <span
                v-if="store.loadout[s.id]"
                class="absolute -top-1 -right-1 w-4 h-4 rounded-full bg-black/70 border border-border-default
                       text-[10px] leading-none flex items-center justify-center text-text-muted hover:text-accent-red"
                title="Clear slot"
                @click.stop="store.clearSlot(s.id)">
                ×
              </span>
            </button>
          </div>

          <div class="flex-1 min-h-0 relative">
            <!-- One viewer, both modes: Item = single-piece turntable,
                 Character = the full assembled paper doll. -->
            <TurntableViewer ref="turntable" class="absolute inset-0" />
          </div>
        </div>
        <section
          v-show="store.dyeProfilesOpen"
          id="dye-profile-creator"
          aria-labelledby="dye-profile-title"
          class="absolute inset-0 z-20 bg-surface-base overflow-y-auto p-4"
          @keydown.esc.stop="store.dyeProfilesOpen = false">
          <div class="flex flex-col gap-4 max-w-2xl mx-auto">
            <div class="flex items-center justify-between gap-2">
              <h2 id="dye-profile-title" class="text-sm font-semibold text-text-secondary">Dye Profiles</h2>
              <button type="button" class="btn-secondary text-xs" @click="store.dyeProfilesOpen = false">Back to model</button>
            </div>
            <DyeProfiles />
          </div>
        </section>
      </div>

      <!-- Right: customize + saved loadouts -->
      <template #right>
        <div class="flex flex-col h-full min-h-0">
          <!-- Top: per-item customization (dye) -->
          <div class="flex-1 min-h-0 overflow-y-auto p-2 flex flex-col gap-4">
            <div v-if="store.resolved">
              <div class="text-sm font-medium text-text-primary">{{ store.resolved.item_name }}</div>
              <div class="text-[10px] text-text-muted mt-0.5">
                {{ store.resolved.source_field }}
              </div>
            </div>
            <div v-else class="text-xs text-text-dim italic">Select an item for this slot.</div>
            <DyeControls v-if="showDyeIndicators" @dye="onDye" />
            <section v-if="store.associatedItems.length" class="flex flex-col gap-2 min-h-0">
              <h3 class="text-xs font-semibold text-text-secondary uppercase tracking-wide">Equipment using this model</h3>
              <template v-if="showDyeIndicators">
                <p class="text-[11px] text-text-muted">🟢 Dyeable · 🔴 Not dyeable</p>
                <label class="flex items-center gap-1.5 text-xs text-text-muted cursor-pointer">
                  <input v-model="hideUndyeable" type="checkbox" />
                  Hide equipment that cannot be dyed
                </label>
              </template>
              <ul class="max-h-64 overflow-y-auto flex flex-col gap-2" aria-label="Equipment using this model">
                <li v-for="item in equipmentItems" :key="item.id" class="flex items-center gap-2 text-xs">
                  <GameIcon :icon-id="item.icon_id" :alt="item.name" size="sm" />
                  <span class="flex-1 min-w-0 break-words">{{ item.name }}</span>
                  <span v-if="showDyeIndicators" class="w-2.5 h-2.5 rounded-full shrink-0"
                    :style="{ backgroundColor: item.not_dyeable ? '#ef4444' : '#22c55e' }"
                    :title="item.not_dyeable ? 'Not dyeable' : 'Dyeable'"
                    role="img" :aria-label="item.not_dyeable ? 'Not dyeable' : 'Dyeable'" />
                </li>
              </ul>
              <p v-if="!equipmentItems.length" class="text-xs text-text-muted">No dyeable equipment. Turn off the filter to show all equipment.</p>
            </section>
          </div>

          <!-- Bottom: saved loadouts -->
          <div
            class="shrink-0 max-h-[45%] overflow-y-auto p-2 border-t border-border-default flex flex-col gap-2">
            <span class="text-xs font-semibold text-text-secondary uppercase tracking-wide">
              Loadouts
            </span>
            <div class="flex gap-1">
              <input
                v-model="presetName"
                type="text"
                placeholder="Name this loadout…"
                class="input text-xs flex-1 min-w-0"
                @keyup.enter="saveCurrent" />
              <button
                class="btn-secondary text-xs px-2 whitespace-nowrap disabled:opacity-40 disabled:cursor-not-allowed"
                :disabled="!canSave"
                :title="canSave ? 'Save current loadout' : 'Equip at least one item first'"
                @click="saveCurrent">
                Save
              </button>
            </div>
            <div v-if="store.presets.length === 0" class="text-[11px] text-text-dim italic">
              No saved loadouts yet.
            </div>
            <div
              v-for="p in store.presets"
              :key="p.id"
              class="flex items-center gap-1 text-xs group">
              <button
                class="flex-1 min-w-0 text-left truncate px-1.5 py-1 rounded hover:bg-white/5 hover:text-accent-gold cursor-pointer"
                :title="`Load “${p.name}”`"
                @click="store.loadLoadout(p.id)">
                {{ p.name }}
                <span class="text-[10px] text-text-dim">
                  ({{ p.sex === "f" ? "F" : "M" }} · {{ Object.keys(p.loadout).length }})
                </span>
              </button>
              <button
                class="shrink-0 w-5 h-5 rounded text-text-dim hover:text-accent-red hover:bg-white/5 cursor-pointer leading-none"
                title="Delete loadout"
                @click="store.deleteLoadout(p.id)">
                ×
              </button>
            </div>
          </div>
        </div>
      </template>
    </PaneLayout>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted } from "vue";
import { useModelViewerStore } from "../../../stores/modelViewerStore";
import { VIEWER_SLOTS, type ItemGroup } from "../../../types/modelViewer";
import PaneLayout from "../../Shared/PaneLayout.vue";
import GameIcon from "../../Shared/GameIcon.vue";
import TurntableViewer from "./TurntableViewer.vue";
import DyeControls from "./DyeControls.vue";
import DyeProfiles from "./DyeProfiles.vue";

const store = useModelViewerStore();
const installFolder = ref(store.gameDir);
const turntable = ref<InstanceType<typeof TurntableViewer> | null>(null);
const search = ref("");
// Many cosmetics (holiday hats, masks) have no 3D mesh in the game; hide them
// by default so the list only shows items that actually render.
const onlyInvisible = ref(false);
const hideUndyeable = ref(true);
const showDyeIndicators = computed(() => !["MainHand", "OffHand"].includes(store.activeSlot));
const equipmentItems = computed(() => store.associatedItems.filter(i =>
  !showDyeIndicators.value || !hideUndyeable.value || !i.not_dyeable));
// Gear tiers (Amazing/Astounding/Awesome …) share one appearance — collapse
// items with an identical appearance string into a single "model" row.
const groupModels = ref(true);
const modeledCount = computed(() => store.items.filter((i) => i.has_model).length);

const activeSlotLabel = computed(
  () => VIEWER_SLOTS.find((s) => s.id === store.activeSlot)?.label ?? store.activeSlot,
);

// Prefer a real display name ("Bard Shirt") over internal-style placeholders
// ("BardLeatherChest"): names with spaces win, then shorter names.
function betterRep(a: { name: string }, b: { name: string }): boolean {
  const aSpace = a.name.includes(" ");
  const bSpace = b.name.includes(" ");
  if (aSpace !== bSpace) return aSpace;
  return a.name.length < b.name.length;
}

// All groups for the current slot: one entry per distinct appearance string
// (or per item when grouping is off / an item has no key). The representative
// is the friendliest, shortest name — usually the base tier ("Leather Shirt").
const allGroups = computed<ItemGroup[]>(() => {
  const visibleItems = store.items.filter(i => onlyInvisible.value ? i.invisible : i.has_model);
  if (!groupModels.value) {
    return visibleItems.map((i) => ({ rep: i, names: [i.name], ids: [i.id] }));
  }
  const byKey = new Map<string, ItemGroup>();
  for (const i of visibleItems) {
    const key = i.appearance_key ?? `#${i.id}`;
    const g = byKey.get(key);
    if (!g) {
      byKey.set(key, { rep: i, names: [i.name], ids: [i.id] });
    } else {
      g.names.push(i.name);
      g.ids.push(i.id);
      if (betterRep(i, g.rep)) g.rep = i;
    }
  }
  return [...byKey.values()].sort((a, b) => a.rep.name.localeCompare(b.rep.name));
});

const matchedGroups = computed<ItemGroup[]>(() => {
  const q = search.value.trim().toLowerCase();
  return allGroups.value.filter(
    (g) =>
      (!q || g.names.some((n) => n.toLowerCase().includes(q))),
  );
});

function isGroupChosen(g: ItemGroup): boolean {
  const chosen = store.loadout[store.activeSlot]?.ref;
  return chosen != null && g.ids.some((id) => String(id) === chosen);
}

function groupTitle(g: ItemGroup): string {
  const suffix = g.rep.has_model ? "" : " — no 3D model";
  if (g.names.length === 1) return g.rep.name + suffix;
  const shown = g.names.slice(0, 20);
  const more = g.names.length > shown.length ? `\n… +${g.names.length - shown.length} more` : "";
  return `${g.names.length} items share this model:${suffix}\n${shown.join("\n")}${more}`;
}

// Windowed list: render `visibleCount` rows and grow on scroll, so a large
// slot's items are browsable without rendering every row (and icon) up front.
const listEl = ref<HTMLElement | null>(null);
const CHUNK = 200;
const visibleCount = ref(CHUNK);

const displayedGroups = computed(() => matchedGroups.value.slice(0, visibleCount.value));

// Reset the window when the underlying list, query, or filter changes.
watch([search, () => store.items, onlyInvisible, groupModels], () => {
  visibleCount.value = CHUNK;
  if (listEl.value) listEl.value.scrollTop = 0;
});

function onListScroll() {
  const el = listEl.value;
  if (!el) return;
  if (
    el.scrollTop + el.clientHeight >= el.scrollHeight - 240 &&
    visibleCount.value < matchedGroups.value.length
  ) {
    visibleCount.value = Math.min(visibleCount.value + CHUNK, matchedGroups.value.length);
  }
}

function onDye(slot: string, channel: number, hex: string) {
  // Persist the dye in the store (survives rebuilds / saves into presets) and
  // update the live material uniform without a scene rebuild.
  store.setDye(slot, channel, hex);
  turntable.value?.setDye(slot, channel, hex);
}

// ── Saved loadouts ────────────────────────────────────────────────────────────
const presetName = ref("");
const canSave = computed(() => Object.keys(store.loadout).length > 0);

function saveCurrent() {
  if (!canSave.value) return;
  store.saveLoadout(presetName.value);
  presetName.value = "";
}

onMounted(async () => {
  await store.init();
  if (store.cacheReady) await store.selectSlot(store.activeSlot);
});
</script>
