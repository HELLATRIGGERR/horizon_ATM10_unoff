/** ExtendedAE slice 7, the tools: the Pattern Modifier's menu takes processing patterns and scales them by the config's
 * factors (x2, then ÷2 back), and used on an Extended Pattern Provider it puts them in; the Config Modifier's menu sets
 * its number and, used on an interface, multiplies what it stocks up to the item's stack size; ME Packing Tape packs a
 * pattern provider with its patterns into a package that places it back; the Extended Pattern Access Terminal (its kit
 * on AE2's terminal, and its wireless form at an access point) lists the providers. Needs content/extendedae
 * (ae2/scripts/prepare-addon.ts extendedae) and a client built with ae2-extendedae. */
import assert from 'node:assert/strict';
import { WorldFixture, until, type Mode } from './harness';

const mode: Mode = process.argv.includes('--in-frame') ? 'in-frame' : 'server-thread';
const fixture = new WorldFixture('extendedae-tools', mode, { addons: ['extendedae'] });
const MODIFIER = 'extendedae:pattern_modifier', CONFIG = 'extendedae:config_modifier', TAPE = 'extendedae:me_packing_tape';
const PACKAGE = 'extendedae:package', EX_PAT = 'extendedae:ex_pattern_access_part', WIRELESS_EX_PAT = 'extendedae:wireless_ex_pat';
const chat = (reply: any) => (reply.chat ?? []).join('\n');
const slot = (view: any, n: number) => view.slots?.find((s: any) => s.slot === n);
const pattern = (input: string, n: number, output: string, m: number) =>
  `ae2:processing_pattern[ae2:encoded_processing_pattern={sparseInputs:[{"#t":"ae2:i",id:"${input}","#":${n}L}],sparseOutputs:[{"#t":"ae2:i",id:"${output}","#":${m}L}]}]`;
/** The amounts (`"#"`) of the generic stacks in an SNBT text, in order. */
/** The record of item `id` in an SNBT list of item records. */
const stocked = (text: string, id: string) => text.split('}').find(entry => entry.includes(`"${id}"`)) ?? '';
const amounts = (text: string) => [...text.matchAll(/"#":\s*(\d+)L/g)].map(m => Number(m[1]));
try {
  const start = await fixture.start(), [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 24, y = Math.ceil(py) + 5, z = Math.floor(pz) + 24;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`tp @s ${x + 3.5} ${y} ${z - 2.5}`);
  for (const [dx, dz] of [[-3, -4], [-3, 6], [14, -4], [14, 6]] as const)
    await until(() => fixture.raw(`block ${x + dx} ${y} ${z + dz}`), v => v.loaded, 'fixture chunks loaded', 30000);
  await fixture.command(`fill ${at(-3, 0, -4)} ${at(14, 4, 6)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -1, -4)} ${at(14, -1, 6)} minecraft:stone`);
  await fixture.command('clear @s');
  // One grid: power, a drive, an Extended Pattern Provider, AE2's Interface, a cable bus for the terminals, an access point.
  const power = at(0), drive = at(1), provider = at(2), iface = at(3), host = at(4), host2 = at(5), access = at(6);
  for (const [pos, id] of [[power, 'ae2:creative_energy_cell'], [drive, 'ae2:drive'], [provider, 'extendedae:ex_pattern_provider'], [iface, 'ae2:interface'], [host, 'ae2:cable_bus'], [host2, 'ae2:cable_bus'], [access, 'ae2:wireless_access_point[facing=east]']] as const)
    await fixture.command(`setblock ${pos} ${id}`);
  const online = (pos: string, label: string) => until(() => fixture.ae(`grid status ${pos}`), v => v.online && v.powered, label, 30000);
  await online(provider, 'the provider online');
  const record = async (pos: string, path = '') => chat(await fixture.command(`data get block ${pos}${path}`));
  /** Uses the held item on block `pos`'s north face, standing right in front of it. */
  const use = async (pos: string, sneak = false) => {
    const [bx, , bz] = pos.split(' ').map(Number);
    await fixture.command(`tp @s ${bx + 0.5} ${y} ${bz - 1.5}`);
    if (sneak) await fixture.sneakInteract(`${pos} north`); else await fixture.raw(`interact ${pos} north`);
  };
  const selected = async () => chat(await fixture.command('data get entity @s SelectedItem'));

  // The Pattern Modifier: two patterns shift-clicked in, x2, ÷2, x2; then used on the provider.
  await fixture.replace('hotbar.0', MODIFIER);
  await fixture.replace('hotbar.1', pattern('minecraft:stone', 2, 'minecraft:cobblestone', 3));
  await fixture.replace('hotbar.2', pattern('minecraft:dirt', 1, 'minecraft:coarse_dirt', 5));
  await fixture.select(0, MODIFIER);
  await fixture.raw('look 0 -90'); await fixture.raw('use-item');
  let view = await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'pattern_modifier.json', 'Pattern Modifier screen', 15000);
  for (const n of [36, 37, 38, 39, 40, 41, 42, 43, 44, 45]) assert(view.ae2_screen.toolbar?.some((b: any) => b.entry === n), `button ${n}`);
  await fixture.raw('click-slot 1 shift');
  await until(() => fixture.raw('screen'), v => slot(v, 300)?.item === 'ae2:processing_pattern', 'the first pattern in');
  await fixture.raw('click-slot 2 shift');
  await until(() => fixture.raw('screen'), v => slot(v, 301)?.item === 'ae2:processing_pattern', 'the second pattern in');
  // A stone is no pattern: it stays out.
  await fixture.replace('hotbar.3', 'minecraft:stone', 5);
  await fixture.raw('click-slot 3 shift');
  await fixture.raw('wait 5');
  assert(!slot(await fixture.raw('screen'), 302)?.item, 'stone stays out');
  await fixture.pressSetting(37);
  await until(selected, text => amounts(text).join() === '4,6,2,10', 'x2: both patterns doubled');
  await fixture.pressSetting(41);
  await until(selected, text => amounts(text).join() === '2,3,1,5', '÷2: both halved back');
  // ÷2 again: 3 and 1 do not divide, so neither changes.
  await fixture.pressSetting(41);
  await fixture.raw('wait 5');
  assert.equal(amounts(await selected()).join(), '2,3,1,5', 'an amount ÷2 cannot take leaves its pattern');
  await fixture.pressSetting(37);
  await until(selected, text => amounts(text).join() === '4,6,2,10', 'x2 again');
  await fixture.raw('close');
  await use(provider);
  await until(() => record(provider), text => amounts(text).join() === '4,6,2,10', 'the provider took both scaled patterns', 15000);
  assert.equal(amounts(await selected()).length, 0, 'the modifier gave them all');
  fixture.mark('the Pattern Modifier scales processing patterns by its factors and puts them into a provider');

  // The Config Modifier: x1 by default; +1 makes it x2. Used on the interface: 3 diamonds become 6, 40 stone 64.
  await fixture.command(`data modify block ${iface} ae2_lab_config.ae2_interface_config set value [{Slot:0b,id:"minecraft:diamond",count:3},{Slot:1b,id:"minecraft:stone",count:40}]`);
  await fixture.replace('hotbar.0', CONFIG); await fixture.select(0, CONFIG);
  await fixture.raw('look 0 -90'); await fixture.raw('use-item');
  view = await until(() => fixture.raw('screen'), v => v.ae2_screen?.style === 'config_modifier.json', 'Config Modifier screen', 15000);
  assert.deepEqual([view.ae2_screen.data?.[0], view.ae2_screen.data?.[1]], [2, 1], 'times one');
  await fixture.pressSetting(1);
  await until(() => fixture.raw('screen'), v => v.ae2_screen?.data?.[1] === 2, 'times two');
  await fixture.raw('close');
  await use(iface);
  await until(() => record(iface, ' ae2_lab_config.ae2_interface_config'), text => /count: 6,/.test(stocked(text, 'minecraft:diamond')) && /count: 64,/.test(stocked(text, 'minecraft:stone')), 'the interface stocks 6 diamonds and 64 stone', 15000);
  fixture.mark('the Config Modifier multiplies an interface\'s stock up to the item\'s stack size');

  // ME Packing Tape, in survival: the provider and its patterns into a package, one use of the tape; the package puts it back.
  await fixture.command('gamemode survival @s');
  await fixture.replace('hotbar.0', TAPE); await fixture.select(0, TAPE);
  await use(provider);
  await until(() => fixture.raw(`getblock ${provider}`), v => v.block === 'minecraft:air', 'the provider packed');
  await until(selected, text => /"?minecraft:damage"?:\s*1\b/.test(text), 'one use of the tape');
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z + 0.5}`);
  await until(() => fixture.raw('inventory'), v => (v.counts?.[PACKAGE] ?? 0) === 1, 'the package picked up', 15000);
  await fixture.command(`tp @s ${x + 3.5} ${y} ${z - 2.5}`);
  const hotbar = (await fixture.raw('inventory')).hotbar.findIndex((s: any) => s?.item === PACKAGE);
  assert(hotbar >= 0, 'the package in the hotbar');
  await fixture.select(hotbar, PACKAGE);
  await fixture.command(`tp @s ${x + 2.5} ${y} ${z - 1.5}`);
  await fixture.raw(`interact ${at(2, -1)} up`);
  await until(() => fixture.raw(`getblock ${provider}`), v => v.block.startsWith('extendedae:ex_pattern_provider'), 'the provider back');
  assert.equal(amounts(await record(provider)).join(), '4,6,2,10', 'with its patterns');
  assert.equal((await fixture.raw('inventory')).counts?.[PACKAGE] ?? 0, 0, 'the package used up');
  await online(provider, 'the provider back on the grid');
  await fixture.command('gamemode creative @s');
  fixture.mark('ME Packing Tape packs a provider with its patterns, and the package puts it back');

  // The Extended Pattern Access Terminal: AE2's terminal on the cable, the kit (sneaking) makes it the extended one.
  await fixture.replace('hotbar.0', 'ae2:pattern_access_terminal'); await fixture.select(0, 'ae2:pattern_access_terminal');
  assert.equal((await fixture.ae(`grid part ${host} north`)).attached, true);
  await fixture.replace('hotbar.0', 'extendedae:pattern_terminal_upgrade'); await fixture.select(0, 'extendedae:pattern_terminal_upgrade');
  await use(host, true);
  await until(() => record(host), text => text.includes(EX_PAT), 'the kit made it the extended terminal', 15000);
  await fixture.replace('hotbar.0', EX_PAT); await fixture.select(0, EX_PAT);
  assert.equal((await fixture.ae(`grid part ${host2} north`)).attached, true);
  await until(() => fixture.ae(`grid part-status ${host2} north`), v => v.online, 'the placed terminal online', 30000);
  const listed = (v: any) => /Pattern Access/i.test(JSON.stringify(v.title ?? '')) && v.slots?.filter((s: any) => s.slot >= 300 && s.slot < 354 && s.item === 'ae2:processing_pattern').length === 2;
  for (const target of [host, host2]) {
    await fixture.replace('hotbar.0', 'minecraft:stick'); await fixture.select(0, 'minecraft:stick');
    await use(target);
    await until(() => fixture.raw('screen'), listed, `the terminal at ${target} lists the provider's two patterns`, 15000);
    await fixture.raw('close');
  }
  // Its wireless form at the access point.
  const link = `ae2:wireless_link_target={dimension:"minecraft:overworld",pos:[I;${x + 6},${y},${z}]},ae2:stored_energy=1000000.0d`;
  await fixture.replace('hotbar.0', `${WIRELESS_EX_PAT}[${link}]`); await fixture.select(0, WIRELESS_EX_PAT);
  await online(access, 'the access point online');
  await fixture.raw('look 0 -90'); await fixture.raw('use-item');
  await until(() => fixture.raw('screen'), listed, 'the wireless terminal lists them too', 15000);
  await fixture.raw('close');
  fixture.mark('the Extended Pattern Access Terminal, its kit and its wireless form list the providers');
  fixture.report('passed');
} catch (error) {
  fixture.report('failed', error);
  throw error;
} finally {
  await fixture.close();
}
