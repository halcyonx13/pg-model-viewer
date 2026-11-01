// Store-level integration check with the Tauri command boundary mocked.
// Run: node tools/test_install_flow.cjs
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('typescript');

const calls = [];
const saved = new Map();
const custom = 'D:\\Games\\Project Gorgon';
let extracted = false;
let channels = 3;
let extractionFailure = null;
const equipment = [
  { id: 10, name: 'Undyeable shirt', equip_slot: 'Chest', appearance_key: 'shirt', not_dyeable: true },
  { id: 11, name: 'Dyeable shirt', equip_slot: 'Chest', appearance_key: 'shirt', not_dyeable: false },
  { id: 12, name: 'Undyeable hat', equip_slot: 'Head', appearance_key: 'hat', not_dyeable: true },
  { id: 13, name: 'Sword', equip_slot: 'MainHand', appearance_key: 'sword', not_dyeable: false },
  { id: 14, name: 'Invisible hand', equip_slot: 'OffHand', appearance_key: null, invisible: true },
  { id: 15, name: 'Other invisible hand', equip_slot: 'OffHand', appearance_key: null, invisible: true },
];
saved.set('pgmv:loadouts', JSON.stringify([{
  id: 'old', name: 'Old preset', sex: 'm',
  loadout: { Chest: { ref: '10', name: 'Old name' }, Head: { ref: '999', name: 'NPC gear' } },
  dye: { Head: { 1: '#ff0000' } },
}]));
const source = fs.readFileSync('src/stores/modelViewerStore.ts', 'utf8')
  .replace(/^import[\s\S]*?;\r?\n/gm, '')
  .replace('export const useModelViewerStore', 'const useModelViewerStore')
  .split('if (import.meta.hot)')[0] + '\nglobalThis.makeStore = useModelViewerStore;';
const context = vm.createContext({
  defineStore: (_name, setup) => setup,
  ref: value => ({ value }),
  computed: getter => ({ get value() { return getter(); } }),
  localStorage: { getItem: key => saved.get(key), setItem: (key, value) => saved.set(key, value) },
  listen: async () => () => {},
  invoke: async (name, args) => {
    calls.push({ name, args });
    if (name === 'model_viewer_status') return {
      platform: 'windows',
      game_bundles_dir: args.gameDir === custom ? custom + '/WindowsPlayer_Data/StreamingAssets/aa/StandaloneWindows64' : null,
      cache_ready: extracted,
    };
    if (name === 'start_model_extraction') {
      if (extractionFailure) throw extractionFailure;
      assert.equal(args.gameDir, custom);
      extracted = true;
      return 'done';
    }
    if (name === 'model_cache_root') return 'cache';
    if (name === 'resolve_item_appearance') return {
      item_id: Number(args.reference),
      slots: [{ slot: 'Chest', dyeable: true, dye_channels: channels, is_weapon: Number(args.reference) === 13 }],
    };
    if (name === 'list_appearance_items') return equipment;
    return [];
  },
});
vm.runInContext(ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2020 } }).outputText, context);

(async () => {
  const store = context.makeStore();
  await store.init();
  assert.equal(store.modelCanDye(10), true, 'shared model supports dye even when representative does not');
  assert.equal(store.modelCanDye(12), false, 'entirely undyeable model');
  assert.equal(store.modelCanDye(13), false, 'weapons never offer dyes');
  assert.equal(store.presets.value[0].loadout.Head, undefined, 'removed NPC gear from old presets');
  assert.equal(store.presets.value[0].loadout.Chest.name, 'Undyeable shirt');
  store.selectedRef.value = '10';
  assert.equal(store.associatedItems.value.length, 2);
  store.selectedRef.value = '14';
  assert.equal(store.associatedItems.value.length, 1, 'missing appearance does not group unrelated items');
  assert.equal(store.gameFound.value, false);
  await store.useGameDirectory('Z:\\Invalid');
  assert.match(store.error.value, /WindowsPlayer.exe/);
  assert.equal(saved.has('pgmv:game-dir'), false);
  await store.useGameDirectory('  "' + custom + '"  ');
  assert.equal(store.gameFound.value, true);
  assert.equal(saved.get('pgmv:game-dir'), custom);
  await store.runExtraction();
  assert.equal(store.error.value, null);
  assert.equal(store.cacheReady.value, true);
  assert.equal(store.extracting.value, false);
  assert.ok(calls.some(call => call.name === 'resolve_base_body'));
  assert.ok(calls.some(call => call.name === 'list_appearance_items'));
  extractionFailure = { message: 'Unity version missing', request_unity_version: true };
  await store.runExtraction();
  assert.equal(store.needsUnityVersion.value, true);
  assert.equal(store.error.value, 'Unity version missing');
  assert.equal(store.extracting.value, false);
  const attempted = calls.length;
  store.unityVersion.value = 'not-a-version';
  await store.runExtraction();
  assert.equal(calls.length, attempted, 'invalid version never launches extraction');
  store.unityVersion.value = ' 6000.3.11f1 ';
  await store.runExtraction();
  assert.equal(calls.filter(c => c.name === 'start_model_extraction').at(-1).args.unityVersion, '6000.3.11f1');
  assert.equal(store.needsUnityVersion.value, true, 'failed retry keeps the prompt');
  extractionFailure = null;
  await store.runExtraction();
  assert.equal(store.needsUnityVersion.value, false);
  extractionFailure = { message: 'Windows extractor error', request_unity_version: false };
  await store.runExtraction();
  assert.equal(store.needsUnityVersion.value, false, 'Windows failures do not trigger manual prompt');
  extractionFailure = null;
  console.log('Unity-version prompt, validation, retry argument, repeated failures and Windows behavior: passed');
  const reopened = context.makeStore();
  await reopened.init();
  assert.equal(reopened.gameDir.value, custom);
  assert.equal(reopened.gameFound.value, true);
  const palette = [{ name: 'Red', color: '#ff0000' }, { name: 'Green', color: '#00ff00' }, { name: 'Blue', color: '#0000ff' }];
  assert.equal(store.saveDyeProfile('Incomplete', palette.slice(0, 2)), false);
  assert.equal(store.saveDyeProfile('  ', palette), false);
  assert.equal(store.saveDyeProfile(' RGB ', palette), true);
  palette[0].color = '#ffffff';
  const restored = context.makeStore();
  assert.equal(restored.dyeProfiles.value[0].name, 'RGB');
  assert.equal(restored.dyeProfiles.value[0].dyes[0].color, '#ff0000', 'saved profiles own a color snapshot');
  const profileId = store.dyeProfiles.value[0].id;
  assert.equal(store.setAutoApplyDefaultDyeProfile(true), false, 'cannot enable without a default');
  assert.equal(store.setDefaultDyeProfile(profileId), true);
  assert.equal(store.setAutoApplyDefaultDyeProfile(true), true);
  for (channels of [1, 2, 3]) {
    await store.chooseItem(equipment[0]);
    assert.equal(Object.keys(store.dyeBySlot.value.Chest).length, channels);
    assert.equal(store.getDye('Chest', 1), '#ff0000');
  }
  assert.equal(store.saveDyeProfile('RGB', palette, profileId), true);
  assert.equal(store.dyeProfiles.value.length, 1, 'same name overwrites existing id');
  assert.equal(store.dyeProfiles.value[0].id, profileId);
  assert.equal(store.getDye('Chest', 1), '#ffffff', 'editing active default updates current dyes');
  assert.equal(store.saveDyeProfile('Copy', palette, profileId), true);
  assert.equal(store.dyeProfiles.value.length, 2, 'renaming creates a new profile');
  const preferencesRestored = context.makeStore();
  assert.equal(preferencesRestored.defaultDyeProfileId.value, profileId);
  assert.equal(preferencesRestored.autoApplyDefaultDyeProfile.value, true);
  await store.chooseItem(equipment[2]);
  assert.equal(store.getDye('Chest', 1), undefined, 'no default on undyeable model');
  await store.chooseItem(equipment[3]);
  assert.equal(store.getDye('Chest', 1), undefined, 'no default on weapons');
  store.setAutoApplyDefaultDyeProfile(false);
  await store.chooseItem(equipment[0]);
  assert.equal(store.getDye('Chest', 1), undefined, 'unchecked auto-apply leaves defaults');
  store.deleteDyeProfile(profileId);
  assert.equal(store.defaultDyeProfileId.value, null);
  assert.equal(context.makeStore().autoApplyDefaultDyeProfile.value, false);
  store.deleteDyeProfile(store.dyeProfiles.value[0].id);
  console.log('Default profile persistence, channel counts, overwrite/save-as-new and eligibility: passed');
  assert.equal(restored.deleteDyeProfile(restored.dyeProfiles.value[0].id), true);
  assert.equal(context.makeStore().dyeProfiles.value.length, 0);
  saved.set('pgmv:dye-profiles', '{broken');
  assert.equal(context.makeStore().dyeProfiles.value.length, 0);
  console.log('Dye profile validation, persistence, deletion and corrupt-storage recovery: passed');
  console.log('Install prompt → validation → persistence → extraction → viewer refresh: passed');
})().catch(error => { console.error(error); process.exitCode = 1; });
