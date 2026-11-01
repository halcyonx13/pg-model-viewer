// Exercise the Vue panels' computed rules without a running Tauri window.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('typescript');
const { parse } = require('@vue/compiler-sfc');
const base = 'src/components/Character/ModelViewer/';
const ref = value => ({ value });
const computed = getter => ({ get value() { return getter(); } });
function panel(file, store, expose, emit = () => {}) {
  const script = parse(fs.readFileSync(base + file, 'utf8')).descriptor.scriptSetup.content
    .replace(/^import[\s\S]*?;\r?\n/gm, '');
  const context = vm.createContext({
    ref, computed, watch() {}, onMounted() {}, defineEmits: () => emit,
    useModelViewerStore: () => store, VIEWER_SLOTS: [],
  });
  vm.runInContext(ts.transpileModule(script + `\nglobalThis.result = {${expose}};`, {
    compilerOptions: { target: ts.ScriptTarget.ES2020 },
  }).outputText, context);
  return context.result;
}
const shirt = { id: 1, name: 'Shirt', appearance_key: 'shirt', has_model: true, invisible: false, not_dyeable: true };
const dyeable = { ...shirt, id: 2, name: 'Dyeable Shirt', not_dyeable: false };
const invisible = { id: 3, name: 'Invisible', appearance_key: 'none', has_model: false, invisible: true };
const missing = { id: 4, name: 'Missing mesh', appearance_key: 'missing', has_model: false, invisible: false };
const store = { gameDir: '', activeSlot: 'Chest', items: [shirt, dyeable, invisible, missing], associatedItems: [shirt, dyeable] };
const list = panel('ModelViewerScreen.vue', store,
  'matchedGroups, onlyInvisible, equipmentItems, hideUndyeable, search, showDyeIndicators');
assert.equal(list.matchedGroups.value.length, 1, 'default hides absent meshes');
assert.equal(list.matchedGroups.value[0].ids.length, 2, 'same appearance grouped');
list.onlyInvisible.value = true;
assert.equal(list.matchedGroups.value[0].rep.id, 3, 'invisible equipment shown without mesh');
assert.equal(list.equipmentItems.value.length, 1, 'undyeable equipment hidden initially');
list.hideUndyeable.value = false;
assert.equal(list.equipmentItems.value.length, 2);
store.activeSlot = 'MainHand';
assert.equal(list.showDyeIndicators.value, false, 'main hand hides the dye panel and indicators');
list.hideUndyeable.value = true;
assert.equal(list.equipmentItems.value.length, 2, 'weapon equipment unaffected by dye filter');
store.activeSlot = 'OffHand';
assert.equal(list.showDyeIndicators.value, false, 'offhand hides the dye panel and indicators');

let canDye = false;
const armorSlot = { slot: 'Chest', dyeable: true, dye_channels: 3, is_weapon: false };
const dyes = panel('DyeControls.vue', {
  viewMode: 'item', resolved: { item_id: 1, slots: [armorSlot] },
  modelCanDye: () => canDye,
}, 'dyeableSlots, notDyeable');
assert.equal(dyes.dyeableSlots.value.length, 0);
assert.equal(dyes.notDyeable.value, true);
canDye = true;
assert.equal(dyes.dyeableSlots.value.length, 1, 'shared dyeable model exposes channels');
armorSlot.is_weapon = true;
assert.equal(dyes.dyeableSlots.value.length, 0, 'weapons omit dye channels');
console.log('Equipment list, invisible filter, shared-model dye eligibility: passed');

const palette = [
  { name: 'Red', color: '#ff0000' },
  { name: 'Green', color: '#00ff00' },
  { name: 'Blue', color: '#0000ff' },
];
const emitted = [];
const slot = { slot: 'Chest', dyeable: true, dye_channels: 3, is_weapon: false,
  default_colors: { _Color1: [1, 0, 0], _Color2: [0, 1, 0], _Color3: [0, 0, 1] } };
const dyeStore = {
  viewMode: 'item', resolved: { item_id: 1, slots: [slot] }, modelCanDye: () => true,
  dyeProfiles: [{ id: 'rgb', name: 'RGB', dyes: palette }], getDye: () => undefined,
};
const controls = panel('DyeControls.vue', dyeStore, 'applyProfile, reorder, startDrag, dropDye',
  (...args) => emitted.push(args));
for (const hand of ['MainHand', 'OffHand']) {
  dyeStore.activeSlot = hand;
  dyeStore.viewMode = 'character';
  emitted.length = 0;
  controls.applyProfile(slot, { target: { value: 'rgb' } });
  assert.equal(emitted.length, 0, 'hand selection suppresses outfit dye controls too');
}
dyeStore.activeSlot = 'Chest';
dyeStore.viewMode = 'item';
for (const channels of [1, 2, 3]) {
  emitted.length = 0;
  slot.dye_channels = channels;
  const target = { value: 'rgb' };
  controls.applyProfile(slot, { target });
  assert.equal(target.value, '');
  assert.deepEqual(emitted.map(e => e[3]), palette.slice(0, channels).map(d => d.color));
}
emitted.length = 0;
controls.reorder(slot, 1, 3);
assert.deepEqual(emitted.map(e => e[3]), ['#00ff00', '#0000ff', '#ff0000'], 'drag moves rather than swaps dyes');
emitted.length = 0;
controls.startDrag({ ...slot, slot: 'Feet' }, 1, {});
controls.dropDye(slot, 2);
assert.equal(emitted.length, 0, 'cannot reorder dyes across equipment pieces');
controls.startDrag(slot, 3, {});
controls.dropDye(slot, 1);
assert.deepEqual(emitted.map(e => e[3]), ['#0000ff', '#ff0000', '#00ff00']);
emitted.length = 0;
slot.is_weapon = true;
controls.applyProfile(slot, { target: { value: 'rgb' } });
assert.equal(emitted.length, 0, 'cannot apply profiles to weapons');
const saves = [];
const editorStore = { dyes: palette, dyeProfiles: [{ id: 'rgb', name: 'RGB', dyes: palette }], saveDyeProfile: (...args) => { saves.push(args); return true; } };
const editor = panel('DyeProfiles.vue', editorStore, 'choose, selected, canSave, name, save, edit, overwriting, startDrag, dropDye');
editor.name.value = 'RGB';
assert.equal(editor.canSave.value, false);
palette.forEach(d => editor.choose(d));
assert.equal(editor.canSave.value, true);
assert.deepEqual(Array.from(editor.selected.value, d => d.color), palette.map(d => d.color));
editor.save();
assert.equal(editor.canSave.value, false);
editor.edit(editorStore.dyeProfiles[0]);
assert.equal(editor.name.value, 'RGB');
assert.equal(editor.overwriting.value, true);
editor.startDrag(0, {});
editor.dropDye(2);
assert.deepEqual(Array.from(editor.selected.value, d => d.color), ['#00ff00', '#0000ff', '#ff0000']);
assert.equal(palette[0].color, '#ff0000', 'editing does not mutate saved profile');
editor.save();
assert.equal(saves.at(-1)[2], 'rgb', 'overwrite passes original id');
editor.edit(editorStore.dyeProfiles[0]);
editor.name.value = 'New palette';
assert.equal(editor.overwriting.value, false, 'changed name offers Save new');
console.log('Profile editor, channel limits, default-color reordering and drag boundaries: passed');
