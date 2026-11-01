<template>
  <div class="flex flex-col gap-3">
    <div
      v-for="slot in dyeableSlots"
      :key="slot.slot"
      class="flex flex-col gap-1.5">
      <div class="flex items-center justify-between">
        <span class="text-xs font-medium text-text-secondary">{{ slot.slot }}</span>
        <span class="text-[10px] text-text-muted uppercase tracking-wide">
          {{ channelLabel(slot.dye_channels) }} dye
        </span>
      </div>

      <select v-if="store.dyeProfiles.length" class="input text-xs" :aria-label="`Apply dye profile to ${slot.slot}`" value=""
        @change="applyProfile(slot, $event)">
        <option value="" disabled>Apply dye profile…</option>
        <option v-for="profile in store.dyeProfiles" :key="profile.id" :value="profile.id">{{ profile.name }}</option>
      </select>
      <p v-if="slot.dye_channels > 1" class="text-[10px] text-text-muted">Drag the handles to reorder dyes, or use the arrow buttons.</p>

      <!-- One dropdown per dye channel, listing every in-game dye -->
      <div
        v-for="ch in slot.dye_channels"
        :key="ch"
        class="flex items-center gap-2 rounded"
        :class="{ 'bg-accent-gold/15': drag?.slot === slot.slot && dropChannel === ch }"
        @dragover="dragOver(slot, ch, $event)" @drop.prevent="dropDye(slot, ch)" @dragend="clearDrag">
        <button v-if="slot.dye_channels > 1" type="button" draggable="true"
          class="cursor-grab text-text-muted text-sm px-0.5" :aria-label="`Drag dye channel ${ch} in ${slot.slot}`"
          @dragstart="startDrag(slot, ch, $event)" @dragend="clearDrag">⠿</button>
        <span class="text-[10px] text-text-muted w-3 text-center shrink-0">{{ ch }}</span>
        <span
          class="w-6 h-6 rounded border border-white/15 shrink-0"
          :style="{ backgroundColor: displayHex(slot, ch) }"
          :title="displayHex(slot, ch)" />
        <select
          class="input text-xs flex-1 min-w-0"
          :value="selectedValue(slot, ch)"
          @change="onSelect(slot, ch, $event)">
          <option value="">Default ({{ defaultHex(slot, ch) }})</option>
          <option v-for="d in store.dyes" :key="d.name" :value="normalize(d.color)">
            {{ d.name }}
          </option>
        </select>
        <div v-if="slot.dye_channels > 1" class="flex flex-col text-[10px] text-text-muted">
          <button type="button" class="cursor-pointer disabled:opacity-25" :disabled="ch === 1" :aria-label="`Move dye ${ch} up in ${slot.slot}`" @click="reorder(slot, ch, ch - 1)">▲</button>
          <button type="button" class="cursor-pointer disabled:opacity-25" :disabled="ch === slot.dye_channels" :aria-label="`Move dye ${ch} down in ${slot.slot}`" @click="reorder(slot, ch, ch + 1)">▼</button>
        </div>
      </div>
    </div>

    <div v-if="dyeableSlots.length === 0" class="text-xs text-text-muted">
      {{
        notDyeable
          ? "Not dyeable"
          : store.viewMode === "character"
          ? "No dyeable pieces equipped."
          : "No dye channels."
      }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useModelViewerStore } from "../../../stores/modelViewerStore";
import type { ResolvedSlot } from "../../../types/modelViewer";

const store = useModelViewerStore();
const drag = ref<{ slot: string; channel: number } | null>(null);
const dropChannel = ref<number | null>(null);

const emit = defineEmits<{
  (e: "dye", slot: string, channel: number, hex: string): void;
}>();

// Item mode: the active item's dye channels. Character mode: every dyeable
// piece across the whole equipped loadout, so the outfit is dyeable in one
// place without re-selecting each slot.
const dyeableSlots = computed<ResolvedSlot[]>(() => {
  if (["MainHand", "OffHand"].includes(store.activeSlot)) return [];
  const source =
    store.viewMode === "character"
      ? Object.values(store.resolvedLoadout).filter(a => store.modelCanDye(a.item_id)).flatMap((a) => a.slots)
      : (store.resolved && store.modelCanDye(store.resolved.item_id) ? store.resolved.slots : []);
  return source.filter((s) => !s.is_weapon && s.dyeable && s.dye_channels > 0);
});

const notDyeable = computed(
  () => store.viewMode === "item" && !!store.resolved && !store.modelCanDye(store.resolved.item_id),
);

watch(dyeableSlots, clearDrag);

function channelLabel(n: number): string {
  return n === 1 ? "Single" : n === 2 ? "Double" : n === 3 ? "Triple" : `${n}`;
}

/** Game DyeColor values are bare uppercase hex ("00BFFF") → "#00bfff". */
function normalize(hex: string): string {
  return `#${hex.replace(/^#/, "").toLowerCase()}`;
}

function defaultHex(slot: ResolvedSlot, channel: number): string {
  const c = slot.default_colors?.[`_Color${channel}`];
  if (!c) return "#808080";
  const to = (v: number) =>
    Math.max(0, Math.min(255, Math.round(v * 255)))
      .toString(16)
      .padStart(2, "0");
  return `#${to(c[0])}${to(c[1])}${to(c[2])}`;
}

/** The swatch chip: the user's stored dye for this slot/channel, else default. */
function displayHex(slot: ResolvedSlot, channel: number): string {
  return store.getDye(slot.slot, channel) ?? defaultHex(slot, channel);
}

/** Dropdown selection: stored dye hex if it matches a game dye, else Default. */
function selectedValue(slot: ResolvedSlot, channel: number): string {
  const stored = store.getDye(slot.slot, channel);
  if (!stored) return "";
  const hex = normalize(stored);
  return store.dyes.some((d) => normalize(d.color) === hex) ? hex : "";
}

function onSelect(slot: ResolvedSlot, channel: number, ev: Event) {
  const value = (ev.target as HTMLSelectElement).value;
  // "" = revert to the material's own default color for this channel.
  emit("dye", slot.slot, channel, value || defaultHex(slot, channel));
}

function applyProfile(slot: ResolvedSlot, event: Event) {
  const select = event.target as HTMLSelectElement;
  const profile = store.dyeProfiles.find(p => p.id === select.value);
  if (profile && dyeableSlots.value.includes(slot)) {
    profile.dyes.slice(0, slot.dye_channels).forEach((dye, i) => emit("dye", slot.slot, i + 1, dye.color));
  }
  select.value = "";
}

function reorder(slot: ResolvedSlot, from: number, to: number) {
  if (!dyeableSlots.value.includes(slot) || from === to || from < 1 || to < 1
    || from > slot.dye_channels || to > slot.dye_channels) return;
  const colors = Array.from({ length: slot.dye_channels }, (_, i) => displayHex(slot, i + 1));
  colors.splice(to - 1, 0, colors.splice(from - 1, 1)[0]);
  colors.forEach((hex, i) => emit("dye", slot.slot, i + 1, hex));
}

function startDrag(slot: ResolvedSlot, channel: number, event: DragEvent) {
  drag.value = { slot: slot.slot, channel };
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", `${slot.slot}:${channel}`);
  }
}

function dragOver(slot: ResolvedSlot, channel: number, event: DragEvent) {
  if (drag.value?.slot !== slot.slot) return;
  event.preventDefault();
  if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
  dropChannel.value = channel;
}

function dropDye(slot: ResolvedSlot, channel: number) {
  if (drag.value?.slot === slot.slot) reorder(slot, drag.value.channel, channel);
  clearDrag();
}

function clearDrag() {
  drag.value = null;
  dropChannel.value = null;
}
</script>
