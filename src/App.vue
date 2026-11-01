<template>
  <div class="app-root font-mono text-text-primary">
    <header class="app-titlebar">
      <div class="flex items-baseline gap-3 min-w-0">
        <span class="text-accent-gold font-semibold text-sm shrink-0">PG Model Viewer</span>
        <span class="text-text-dim text-[11px] hidden sm:inline truncate">
          Project: Gorgon 3D item models &amp; live dye
        </span>
      </div>
      <div class="flex items-center gap-3">
        <!-- Auto-update banner -->
        <div
          v-if="updateStore.updateAvailable && !updateStore.dismissed"
          class="flex items-center gap-1.5 text-[11px]">
          <span v-if="updateStore.installing" class="text-accent-blue">
            Updating… {{ updateStore.downloadProgress }}%
          </span>
          <template v-else>
            <button
              class="flex items-center gap-1.5 text-accent-blue hover:brightness-125 transition cursor-pointer"
              @click="updateStore.downloadAndInstall()">
              <span class="inline-block w-1.5 h-1.5 rounded-full bg-accent-blue animate-pulse" />
              Update to v{{ updateStore.latestVersion }}
            </button>
            <button
              class="text-text-dim hover:text-text-primary cursor-pointer"
              title="Dismiss"
              @click="updateStore.dismiss()">
              &#10005;
            </button>
          </template>
          <span v-if="updateStore.installError" class="text-accent-red truncate max-w-56">
            {{ updateStore.installError }}
          </span>
        </div>
        <button
          type="button"
          class="px-2 py-1 text-xs rounded border border-border-default text-text-muted hover:bg-white/5 hover:text-text-secondary cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
          :class="{ 'bg-white/5 text-text-secondary': viewerStore.dyeProfilesOpen }"
          :disabled="viewerStore.loading || !viewerStore.gameFound || !viewerStore.cacheReady"
          :aria-expanded="viewerStore.dyeProfilesOpen"
          aria-controls="dye-profile-creator"
          @click="viewerStore.dyeProfilesOpen = !viewerStore.dyeProfilesOpen">
          Dye Profiles
        </button>
        <SettingsMenu />
      </div>
    </header>
    <main class="app-main">
      <ModelViewerScreen />
    </main>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from "vue";
import ModelViewerScreen from "./components/Character/ModelViewer/ModelViewerScreen.vue";
import SettingsMenu from "./components/SettingsMenu.vue";
import { useSettingsStore } from "./stores/settingsStore";
import { useUpdateStore } from "./stores/updateStore";
import { useModelViewerStore } from "./stores/modelViewerStore";

// Apply persisted font + scale on startup; start the auto-update poll.
const settings = useSettingsStore();
const updateStore = useUpdateStore();
const viewerStore = useModelViewerStore();
onMounted(() => {
  settings.applyAll();
  updateStore.startPolling();
});
</script>

<style>
:root {
  color-scheme: dark;
}
html,
body,
#app {
  height: 100%;
  margin: 0;
}
body {
  background: var(--color-surface-base, #1a1a1a);
}
.app-root {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.app-titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 0.4rem 0.75rem;
  border-bottom: 1px solid var(--color-border-default, #333);
  flex: 0 0 auto;
}
.app-main {
  flex: 1 1 auto;
  min-height: 0;
}
</style>
