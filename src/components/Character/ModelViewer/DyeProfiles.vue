<template>
  <section class="flex flex-col gap-2">
    <label class="flex items-center gap-1.5 text-xs text-text-muted">
      <input type="checkbox" :checked="store.autoApplyDefaultDyeProfile" :disabled="!store.defaultDyeProfileId" @change="toggleAuto" />
      Automatically apply default dye profile
    </label>
    <p class="text-[10px] text-text-muted">{{ defaultName ? `Default: ${defaultName}` : 'Choose a default profile to enable automatic application.' }}</p>
    <div class="flex flex-col gap-3 rounded border border-white/10 p-2">
      <div class="flex items-center justify-between">
        <h3 class="text-xs font-semibold text-text-secondary">{{ editingId ? 'Edit dye profile' : 'Create a dye profile' }}</h3>
        <button type="button" class="text-xs text-text-muted cursor-pointer" @click="reset">New profile</button>
      </div>
      <p class="text-[11px] text-text-muted">Choose three dyes, then name and save your profile. Drag selected dyes to reorder them, or use the arrows. The same dye can be used more than once.</p>
      <div class="grid grid-cols-3 gap-1">
        <div v-for="position in 3" :key="position" class="flex flex-col gap-1 rounded"
          :class="{ 'bg-accent-gold/15': dropPosition === position - 1 }"
          @dragover="dragOver(position - 1, $event)" @drop.prevent="dropDye(position - 1)">
          <button type="button" class="flex flex-col items-center gap-1 rounded border p-1 text-[10px] cursor-pointer"
            :draggable="!!selected[position - 1]" @dragstart="startDrag(position - 1, $event)" @dragend="clearDrag"
            :class="active === position - 1 ? 'border-accent-gold text-accent-gold' : 'border-white/10 text-text-muted'"
            :aria-pressed="active === position - 1" @click="active = position - 1">
            <span>{{ position }}</span>
            <span class="w-6 h-6 rounded border border-white/15" :style="{ backgroundColor: selected[position - 1]?.color ?? 'transparent' }" />
            <span class="break-words">{{ selected[position - 1]?.name ?? 'Choose dye' }}</span>
          </button>
          <div class="flex justify-center gap-3 text-xs text-text-muted">
            <button type="button" :disabled="position === 1" class="cursor-pointer disabled:opacity-25" :aria-label="`Move profile dye ${position} left`" @click="reorder(position - 1, position - 2)">←</button>
            <button type="button" :disabled="position === 3" class="cursor-pointer disabled:opacity-25" :aria-label="`Move profile dye ${position} right`" @click="reorder(position - 1, position)">→</button>
          </div>
        </div>
      </div>
      <input v-model="search" class="input text-xs" placeholder="Search dyes…" aria-label="Search dyes" />
      <div class="grid grid-cols-3 gap-1 max-h-56 overflow-y-auto" aria-label="Available dyes">
        <button v-for="dye in matchingDyes" :key="dye.name" type="button" class="flex flex-col items-center gap-1 rounded border border-white/10 p-1.5 text-[10px] text-text-secondary hover:bg-white/5 cursor-pointer"
          :title="dye.name" @click="choose(dye)">
          <span class="w-7 h-7 rounded border border-white/15" :style="{ backgroundColor: color(dye.color) }" />
          <span class="break-words">{{ dye.name }}</span>
        </button>
      </div>
      <p v-if="!matchingDyes.length" class="text-xs text-text-muted">{{ store.dyes.length ? 'No matching dyes.' : 'No dyes available.' }}</p>
      <form class="flex gap-1" @submit.prevent="save">
        <input v-model="name" class="input text-xs flex-1 min-w-0" placeholder="Profile name…" aria-label="Dye profile name" maxlength="100" />
        <button class="btn-secondary text-xs px-2 disabled:opacity-40" :disabled="!canSave">{{ overwriting ? 'Save changes' : 'Save new' }}</button>
      </form>
      <p v-if="editingId" class="text-[11px] text-text-muted">Keep the original name to overwrite this profile. Change the name to save a new profile.</p>
      <h3 class="text-xs font-semibold text-text-secondary">Saved profiles</h3>
      <p v-if="!store.dyeProfiles.length" class="text-xs text-text-muted">No saved dye profiles yet.</p>
      <ul class="flex flex-col gap-2 max-h-40 overflow-y-auto">
        <li v-for="profile in store.dyeProfiles" :key="profile.id" class="flex items-center gap-1 text-xs">
          <button type="button" class="flex flex-1 min-w-0 items-center gap-1 cursor-pointer text-left rounded hover:bg-white/5 p-1"
            :class="{ 'bg-accent-gold/15': editingId === profile.id }" :aria-label="`Edit profile ${profile.name}`" @click="edit(profile)">
            <span v-for="(dye, index) in profile.dyes" :key="index" class="w-4 h-4 rounded border border-white/15 shrink-0"
              :style="{ backgroundColor: dye.color }" :title="`${index + 1}: ${dye.name}`" />
            <span class="flex-1 min-w-0 break-words text-text-secondary">{{ profile.name }}</span>
          </button>
          <button type="button" class="text-[10px] text-text-muted cursor-pointer" :aria-pressed="store.defaultDyeProfileId === profile.id"
            :aria-label="`Set ${profile.name} as default`" @click="makeDefault(profile.id)">{{ store.defaultDyeProfileId === profile.id ? '★ Default' : 'Set default' }}</button>
          <button type="button" class="text-text-muted hover:text-accent-red cursor-pointer px-1" :aria-label="`Delete profile ${profile.name}`" @click="remove(profile.id)">×</button>
        </li>
      </ul>
      <p class="text-[11px] text-text-muted">Click a saved profile to edit it. Apply profiles using the selector above a piece’s dye channels.</p>
    </div>
    <p v-if="message" role="status" class="text-xs text-text-muted">{{ message }}</p>
  </section>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";
import { useModelViewerStore } from "../../../stores/modelViewerStore";
import type { DyeInfo, ProfileDye, DyeProfile } from "../../../types/modelViewer";

const store = useModelViewerStore();
const active = ref(0);
const selected = ref<(ProfileDye | null)[]>([null, null, null]);
const search = ref("");
const name = ref("");
const message = ref("");
const editingId = ref<string | undefined>();
const draggedPosition = ref<number | null>(null);
const dropPosition = ref<number | null>(null);
const color = (hex: string) => `#${hex.replace(/^#/, "").toLowerCase()}`;
const matchingDyes = computed(() => store.dyes.filter(d => d.name.toLowerCase().includes(search.value.trim().toLowerCase())));
const canSave = computed(() => !!name.value.trim() && selected.value.every(Boolean));
const overwriting = computed(() => store.dyeProfiles.some(p => p.id === editingId.value && p.name === name.value.trim()));
const defaultName = computed(() => store.dyeProfiles.find(p => p.id === store.defaultDyeProfileId)?.name);

function reset() {
  name.value = "";
  selected.value = [null, null, null];
  editingId.value = undefined;
  active.value = 0;
  search.value = "";
  message.value = "";
  clearDrag();
}

function edit(profile: DyeProfile) {
  reset();
  editingId.value = profile.id;
  name.value = profile.name;
  selected.value = profile.dyes.map(d => ({ ...d }));
}

function makeDefault(id: string) {
  message.value = store.setDefaultDyeProfile(id) ? "Default profile updated." : "Couldn't save the default profile.";
}

function toggleAuto(event: Event) {
  const checkbox = event.target as HTMLInputElement;
  if (!store.setAutoApplyDefaultDyeProfile(checkbox.checked)) {
    checkbox.checked = store.autoApplyDefaultDyeProfile;
    message.value = "Couldn't save automatic application preference.";
  }
}

function choose(dye: DyeInfo) {
  selected.value[active.value] = { name: dye.name, color: color(dye.color) };
  const next = selected.value.findIndex((d, i) => !d && i > active.value);
  active.value = next >= 0 ? next : Math.max(0, selected.value.findIndex(d => !d));
  message.value = "";
}

function save() {
  if (!canSave.value) return;
  if (!store.saveDyeProfile(name.value, selected.value as ProfileDye[], editingId.value)) {
    message.value = "Couldn't save the profile. Local storage may be unavailable.";
    return;
  }
  const savedName = name.value.trim();
  reset();
  message.value = `Saved “${savedName}”.`;
}

function remove(id: string) {
  if (!store.deleteDyeProfile(id)) { message.value = "Couldn't delete the profile."; return; }
  if (editingId.value === id) reset();
  message.value = "Profile deleted.";
}

function reorder(from: number, to: number) {
  if (from === to || from < 0 || to < 0 || from > 2 || to > 2) return;
  const next = [...selected.value];
  next.splice(to, 0, next.splice(from, 1)[0]);
  selected.value = next;
  active.value = to;
}

function startDrag(position: number, event: DragEvent) {
  if (!selected.value[position]) { event.preventDefault(); return; }
  draggedPosition.value = position;
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", `profile-dye:${position}`);
  }
}

function dragOver(position: number, event: DragEvent) {
  if (draggedPosition.value === null) return;
  event.preventDefault();
  if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
  dropPosition.value = position;
}

function dropDye(position: number) {
  if (draggedPosition.value !== null) reorder(draggedPosition.value, position);
  clearDrag();
}

function clearDrag() {
  draggedPosition.value = null;
  dropPosition.value = null;
}
</script>
