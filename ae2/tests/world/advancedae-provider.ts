/** Advanced AE slice 1: the Small Advanced Pattern Provider and the Advanced Processing Pattern. The Advanced Pattern
 * Encoder's edits (AdvPatternEncoderMenu.decodeProcessingPattern and update, through `/ae2 advae`) turn AE2's processing
 * pattern of raw iron and coal into an Advanced one and send the raw iron up. The provider (AdvPatternProviderLogic(.., 9))
 * opens Advanced AE's screen and takes the pattern; a craft through the terminal pushes it into the furnace north of the
 * provider (AdvPatternProviderLogic.pushInputsDirectionally): the raw iron through the furnace's top into its input
 * slot, the coal, of no direction, through the side the provider faces into the fuel slot. AE2's Pattern Provider
 * pushes the same pattern as any processing pattern, all to that side, where the furnace refuses raw iron, so the
 * craft waits there. The pattern and its directions survive a restart, and the pattern stays an Advanced one in AE2's provider.
 * Needs content/advanced_ae (ae2/scripts/prepare-addon.ts advancedae) and a client built with ae2-advancedae. */
import assert from 'node:assert/strict';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('advancedae-provider', mode, { addons: ['advanced_ae'] });
const PROVIDER = 'advanced_ae:small_adv_pattern_provider', PATTERN = 'advanced_ae:adv_processing_pattern';
const ae2Pattern = 'ae2:processing_pattern[ae2:encoded_processing_pattern={sparseInputs:[{"#t":"ae2:i",id:"minecraft:raw_iron","#":1L},{"#t":"ae2:i",id:"minecraft:coal","#":1L}],sparseOutputs:[{"#t":"ae2:i",id:"minecraft:iron_ingot","#":1L}]}]';
const cell = 'ae2:item_storage_cell_4k[ae2:storage_cell_inv=[{"#t":"ae2:i",id:"minecraft:raw_iron","#":16L},{"#t":"ae2:i",id:"minecraft:coal","#":16L}]]';
const count = (view: any, item: string) => view.items?.find((entry: any) => entry.item === item)?.count ?? 0;
const slotOf = (view: any, index: number) => view.slots?.find((slot: any) => slot?.slot === index) ?? view.slots?.[index];
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 28, y = Math.ceil(py) + 5, z = Math.floor(pz) + 28;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  for (const [dx, dz] of [[-3, -4], [-3, 4], [9, -4], [9, 4]] as const) {
    await until(async () => (await fixture.command(`execute if loaded ${at(dx, 0, dz)}`)).chat ?? [], lines => lines.includes('Test passed'), 'loaded fixture corner', 30000);
  }
  await fixture.command(`fill ${at(-3, 0, -4)} ${at(9, 4, 4)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -1, -4)} ${at(9, -1, 4)} minecraft:stone`);
  await fixture.command('clear @s');
  // One grid: power, a drive, a terminal, two CPUs (the first job stays on its CPU, waiting for the ingot), the Advanced provider with a furnace north of it, and AE2's provider
  // with a furnace north of it.
  const power = at(0), drive = at(1), terminal = at(2), provider = at(3), furnace = at(3, 0, -1), ae2Provider = at(6), ae2Furnace = at(6, 0, -1);
  for (const [pos, id] of [[power, 'ae2:creative_energy_cell'], [drive, 'ae2:drive'], [terminal, 'ae2:cable_bus'], [at(2, 1), 'ae2:1k_crafting_storage'], [at(0, 1), 'ae2:1k_crafting_storage'],
    [provider, PROVIDER], [furnace, 'minecraft:furnace[facing=north]'], [at(4), 'ae2:cable_bus'], [at(5), 'ae2:cable_bus'], [ae2Provider, 'ae2:pattern_provider'], [ae2Furnace, 'minecraft:furnace[facing=north]']] as const) {
    await fixture.command(`setblock ${pos} ${id}`);
  }
  await fixture.command(`item replace block ${drive} container.0 with ${cell}`);
  await fixture.replace('hotbar.0', 'ae2:terminal'); await fixture.select(0, 'ae2:terminal');
  assert.equal((await fixture.ae(`grid part ${terminal} north`)).attached, true);
  const online = (pos: string) => until(() => fixture.ae(`grid status ${pos}`), view => view.online && view.powered, `online ${pos}`, 30000);
  await online(provider); await online(ae2Provider);
  await until(() => fixture.ae(`grid status ${drive}`), view => count(view, 'minecraft:raw_iron') === 16 && count(view, 'minecraft:coal') === 16, 'the cell is mounted', 30000);

  // The encoder's edits: AE2's pattern becomes an Advanced one with no direction, then the raw iron goes up.
  await fixture.command('clear @s');
  await fixture.command(`item replace entity @s hotbar.1 with ${ae2Pattern}`);
  await fixture.command(`item replace entity @s hotbar.2 with ${ae2Pattern}`);
  await until(() => fixture.raw('inventory'), view => view.counts?.['ae2:processing_pattern'] === 2, 'patterns in the inventory');
  let shown = await fixture.ae('advae encode 1');
  assert.equal(shown.item, PATTERN);
  assert.deepEqual(shown.inputs.map((i: any) => [i.id, i.direction]), [['minecraft:raw_iron', 'none'], ['minecraft:coal', 'none']]);
  shown = await fixture.ae('advae direction 1 minecraft:raw_iron up');
  assert.deepEqual(shown.inputs.map((i: any) => [i.id, i.direction]), [['minecraft:raw_iron', 'up'], ['minecraft:coal', 'none']]);
  await assert.rejects(fixture.ae('advae direction 2 minecraft:raw_iron up'), 'AE2\'s pattern has no directions');
  shown = await fixture.ae('advae encode 2');
  shown = await fixture.ae('advae direction 2 minecraft:raw_iron up');
  assert.equal(shown.inputs[0].direction, 'up');
  fixture.mark('the encoder\'s edits make an Advanced Processing Pattern that sends raw iron up');

  // Advanced AE's screen; the pattern goes into its first pattern slot.
  await fixture.command(`tp @s ${x + 3.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${provider} south`);
  let view = await until(() => fixture.raw('screen'), v => v.ae2_screen, 'Advanced Pattern Provider screen', 15000);
  assert.equal(view.ae2_screen.style, 'small_adv_pattern_provider.json');
  assert(view.ae2_screen.priority, 'priority tab');
  assert.deepEqual(view.ae2_screen.toolbar.map((b: any) => b.entry).filter((e: any) => e !== undefined), [18, 19, 20]);
  const patternSlot = (v: any) => v.slots.find((s: any) => s.slot < 36 && s.item === PATTERN)?.slot;
  await fixture.raw(`click-slot ${patternSlot(view)} shift`);
  view = await until(() => fixture.raw('screen'), v => v.slots.some((s: any) => s.slot >= 300 && s.slot < 309 && s.item === PATTERN), 'the Advanced pattern installed');
  await fixture.raw('close');
  // The same pattern in AE2's provider.
  await fixture.command(`tp @s ${x + 6.5} ${y} ${z + 2.5}`);
  await fixture.raw(`interact ${ae2Provider} south`);
  view = await until(() => fixture.raw('screen'), v => v.slots?.some((s: any) => s.slot < 36 && s.item === PATTERN), 'AE2 provider screen');
  await fixture.raw(`click-slot ${patternSlot(view)} shift`);
  await until(() => fixture.raw('screen'), v => v.slots.some((s: any) => s.slot >= 300 && s.slot < 309 && s.item === PATTERN), 'the pattern in AE2\'s provider');
  await fixture.raw('close');
  fixture.mark('the provider opens Advanced AE\'s screen and takes the Advanced Processing Pattern, as AE2\'s provider does');

  // A craft of one iron ingot: the Advanced provider sends raw iron through the top and coal through the side.
  const request = async () => {
    await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 2.5}`);
    await fixture.raw(`interact ${terminal} north`);
    const entry = (v: any) => v.slots?.find((slot: any) => slot.slot >= 300 && slot.slot < 345 && slot.item === 'minecraft:iron_ingot');
    await until(() => fixture.raw('screen'), entry, 'iron ingot craftable', 30000);
    await until(async () => {
      const v = await fixture.raw('screen');
      if (!JSON.stringify(v.title ?? '').includes('AE2 Craft Amount') && entry(v)) await fixture.raw(`click-slot ${entry(v).slot} middle`);
      return v;
    }, v => JSON.stringify(v.title ?? '').includes('AE2 Craft Amount'), 'craft amount');
    await fixture.raw('click-slot 302');
    const confirm = await until(() => fixture.raw('screen'), v => JSON.stringify(v.title ?? '').includes('AE2 Craft Confirm') && /bytes|Missing materials|craft planning:/.test(JSON.stringify(v)), 'craft confirm');
    assert.doesNotMatch(JSON.stringify(confirm), /Missing materials|craft planning:/);
    await fixture.raw('click-slot 302');
    await fixture.raw('close');
  };
  const furnaceSlots = async (pos: string) => {
    const c = await fixture.raw(`container ${pos}`);
    return [0, 1, 2].map(i => { const s = slotOf(c, i); return s?.item ? `${s.item}x${s.count}` : '-'; });
  };
  // AE2's provider is the one whose pattern a CPU picks second: make the Advanced provider's the only one first by
  // taking AE2's provider off the grid while the first craft runs.
  await fixture.command(`setblock ${at(5)} minecraft:air`);
  await until(() => fixture.ae(`grid status ${ae2Provider}`), v => !v.online, 'AE2\'s provider off the grid', 30000);
  await request();
  // Smelting may already have begun: the raw iron in the input slot, or smelted, which needs the coal in the fuel slot.
  const pushed = await until(() => furnaceSlots(furnace), slots => slots[0].startsWith('minecraft:raw_iron') || slots[2].startsWith('minecraft:iron_ingot'), 'raw iron in the furnace\'s input slot', 60000);
  assert(!pushed[1].startsWith('minecraft:raw_iron'), `no raw iron in the fuel slot: ${pushed}`);
  await until(() => fixture.ae(`grid status ${drive}`), v => count(v, 'minecraft:raw_iron') === 15 && count(v, 'minecraft:coal') === 15, 'both inputs left the grid', 30000);
  await until(() => furnaceSlots(furnace), slots => slots[2].startsWith('minecraft:iron_ingot'), 'the furnace smelts with the coal it got through its side', 60000);
  const data = (await fixture.command(`data get block ${provider}`)).chat?.join('\n') ?? '';
  assert.doesNotMatch(data, /advanced_ae_send_faces/, 'nothing waits in the send list');
  fixture.mark('the Advanced provider pushes raw iron through the furnace\'s top and coal through the side it faces');

  // AE2's provider alone: the same pattern goes all to the side, where the furnace takes no raw iron, so nothing moves.
  await fixture.command(`setblock ${provider} ae2:cable_bus`);
  await fixture.command(`setblock ${at(5)} ae2:cable_bus`);
  await online(ae2Provider);
  await request();
  await until(() => fixture.ae(`grid status ${drive}`), v => count(v, 'minecraft:raw_iron') === 14, 'the second job holds its inputs', 30000);
  await fixture.step(60);
  assert.deepEqual(await furnaceSlots(ae2Furnace), ['-', '-', '-'], 'AE2\'s provider pushes nothing into the furnace\'s side');
  fixture.mark('AE2\'s provider pushes the Advanced pattern as any processing pattern, to one face only');

  // The pattern and its directions survive a restart.
  await fixture.restart();
  const dump = (await fixture.command(`data get block ${ae2Provider}`)).chat?.join('\n') ?? '';
  assert.match(dump, /advanced_ae:adv_processing_pattern/);
  assert.match(dump, /directionMap/);
  assert.match(dump, /"UP"/);
  fixture.mark('the Advanced Processing Pattern and its directions survive a restart');
  fixture.assertHidden(await fixture.raw('status'));
  fixture.report('passed');
} catch (error) { fixture.report('failed', error); throw error; }
finally { await fixture.close(); }
