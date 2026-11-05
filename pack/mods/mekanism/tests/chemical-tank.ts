/** Mekanism's Chemical Tanks and chemical items, as the `mekanism` native bundle and its content mods make them
 * (docs/mekanism-chemicals.md, slice 2), under ATM10's configs. The numbers are those a NeoForge 1.21.1 server with
 * Mekanism 10.7.19 and ATM10's configs gave for the same steps over RCON (`/tick step`, `data get block`):
 *
 * - Tiers: a tank ejects its tier's output a tick out of its front into the tank there: basic 1000, advanced 16 000,
 *   elite 128 000, ultimate 512 000; a target whose touching side is none or output takes nothing, both takes.
 * - Dumping: a basic tank of 64 000 loses 160 a tick (a 400th) and stops ejecting; dumping excess brings it down
 *   1000 a tick to 54 400 (85 %) and still ejects. The creative tank holding 5 mB gives 5 a tick and keeps them.
 * - Its slots: an empty basic tank item in the drain slot fills 1000 a tick; one holding 5000 in the fill slot gives
 *   1000 a tick; the separator's left output slot fills a tank item 1000 a tick and, its item eject on, puts the full
 *   item into a chest on its left; the infuser's infusion slot takes 1000 carbon from a tank item.
 * - Its item: a tank placed from an item holding 12 345 mB of hydrogen holds them and faces the player; broken in
 *   survival, its drop keeps its contents and dump mode; an untouched tank drops a plain item.
 * - Its screen and the dump mode button.
 *
 * Tanks and slots are read from the saved compound (`data get block`) and `/mek probe`. */
import assert from 'node:assert/strict';
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.MEK_CONTENT_DIR ?? join(root, '../.build/content/mekanism'));
const genContent = resolve(process.env.MEKGEN_CONTENT_DIR ?? join(root, '../.build/content/mekanismgenerators'));
const fixture = new WorldFixture('mekanism-chemical-tank', 'server-thread', { devMods: [content, genContent] });
const config = join(fixture.game, 'config/Mekanism');
mkdirSync(config, { recursive: true });
const atm10 = process.env.ATM10_DIR;
if (atm10 && existsSync(join(atm10, 'config/Mekanism/tiers.toml'))) {
  for (const file of ['general.toml', 'tiers.toml', 'machine-usage.toml', 'machine-storage.toml']) copyFileSync(join(atm10, 'config/Mekanism', file), join(config, file));
} else {
  writeFileSync(join(config, 'general.toml'), 'maxUpgradeMultiplier = 16\n[auto_eject]\nchemical = 1024\ndumpExcessKeepRatio = 0.85\n');
}

const H = 'mekanism:hydrogen';
const tank = (tier: string) => `mekanism:${tier}_chemical_tank`;
async function saved(pos: string): Promise<string> {
  const reply = await fixture.command(`data get block ${pos}`);
  const line = (reply.chat ?? []).find((text: string) => text.includes('has the following block data'));
  assert(line, `no block entity at ${pos}: ${JSON.stringify(reply.chat)}`);
  return line;
}
/** A `/mek` answer (`MEK {json}`), the newest. */
async function mek(command: string): Promise<any> {
  const reply = await fixture.command(command);
  const lines = (reply.chat ?? []).filter((t: string) => t.startsWith('MEK {')).map((t: string) => JSON.parse(t.slice(4)));
  assert(lines.length, `${command}: ${JSON.stringify(reply)}`);
  return lines.reduce((a: any, b: any) => (b.n > a.n ? b : a));
}
/** Tank `i` of `chemical_tanks`: its chemical and mB. */
function tankIn(nbt: string, i = 0): [string, number] {
  const list = nbt.match(/chemical_tanks: ?\[(.*?)\]\s*(,|\})/)?.[1] ?? '';
  for (const entry of list.match(/\{stored: ?\{[^{}]*\}, ?tank: ?\d+b\}|\{tank: ?\d+b, ?stored: ?\{[^{}]*\}\}/g) ?? []) {
    if (new RegExp(`tank: ?${i}b`).test(entry)) return [entry.match(/id: ?"([^"]+)"/)?.[1] ?? '', Number(entry.match(/amount: ?(\d+)L/)?.[1] ?? 0)];
  }
  return ['', 0];
}
/** A chemical tank's mB by `/mek probe` (its compound's items may hold chemical tanks of their own). */
const amount = async (pos: string) => (await mek(`mek probe ${pos}`)).amount ?? 0;
const held = (chemical: string, mb: number) => `{chemical_tanks:[{tank:0b,stored:{id:"${chemical}",amount:${mb}L}}]}`;
/** A tank item's SNBT for `Items`: holding `mb` of `chemical`, or empty. */
const item = (slot: number, tier: string, chemical?: string, mb = 0) =>
  `{Slot:${slot}b,id:"${tank(tier)}",count:1${chemical ? `,components:{"minecraft:custom_data":{mekanism:${held(chemical, mb)}}}` : ''}}`;
/** What the tank item in slot `i` of `pos` holds, by `/mek probe`. */
async function slotHeld(pos: string, i: number): Promise<number> {
  const p = await mek(`mek probe ${pos}`);
  return (p.slots ?? [])[i]?.chemical?.amount ?? 0;
}

const log: Record<string, unknown> = {};
try {
  const start = await fixture.start();
  const [px, py, pz] = start.player.position;
  const x = Math.floor(px) + 4, y = Math.min(Math.floor(py) + 2, 78), z = Math.floor(pz) + 6;
  const at = (dx: number, dy = 0, dz = 0) => `${x + dx} ${y + dy} ${z + dz}`;
  await fixture.command(`fill ${at(-3, -1, -4)} ${at(34, 5, 8)} minecraft:air`);
  await fixture.command(`fill ${at(-3, -2, -4)} ${at(34, -2, 8)} minecraft:stone`);
  await fixture.command(`fill ${at(0, -1, 4)} ${at(6, -1, 8)} minecraft:stone`);
  await fixture.hold();

  // Tiers: each tank (front north, ejecting) into an ultimate tank north of it (its back, south, takes).
  const outputs: Record<string, number> = {};
  for (const [i, [tier, mb]] of ([['basic', 30000], ['advanced', 200000], ['elite', 1000000], ['ultimate', 1000000]] as const).entries()) {
    const a = at(3 * i), b = at(3 * i, 0, -1);
    await fixture.command(`setblock ${a} ${tank(tier)}`);
    await fixture.command(`setblock ${b} ${tank('ultimate')}`);
    await fixture.command(`data merge block ${a} ${held(H, mb)}`);
    await fixture.stepServer(1);
    outputs[tier] = await amount(b);
    assert.equal(await amount(a), mb - outputs[tier]);
  }
  log.outputs = outputs;
  assert.deepEqual(outputs, { basic: 1000, advanced: 16000, elite: 128000, ultimate: 512000 });
  fixture.mark('a tank ejects its tier\'s output a tick: 1000, 16 000, 128 000, 512 000');

  // The target's touching side: none or output takes nothing, both takes.
  const s = at(12), t = at(12, 0, -1);
  await fixture.command(`setblock ${s} ${tank('basic')}`);
  await fixture.command(`setblock ${t} ${tank('basic')}`);
  const sides: Record<string, number> = {};
  for (const [name, id] of [['none', 0], ['output', 4], ['input_output', 7]] as const) {
    await fixture.command(`data merge block ${s} ${held(H, 30000)}`);
    await fixture.command(`data merge block ${t} {chemical_tanks:[],component_config:{config2:[I;4,1,1,${id},1,1],eject2:1b}}`);
    await fixture.stepServer(1);
    sides[name] = await amount(t);
  }
  log.sides = sides;
  assert.deepEqual(sides, { none: 0, output: 0, input_output: 1000 });
  fixture.mark('a target side of none or output takes nothing, both takes 1000');

  // Dumping.
  const d = at(15);
  await fixture.command(`setblock ${d} ${tank('basic')}`);
  await fixture.command(`data merge block ${d} {dumping:2,component_config:{config2:[I;0,0,0,0,0,0],eject2:0b}}`);
  await fixture.command(`data merge block ${d} ${held(H, 64000)}`);
  const dumped: number[] = [];
  for (let i = 0; i < 2; i++) { await fixture.stepServer(1); dumped.push(await amount(d)); }
  await fixture.command(`data merge block ${d} {dumping:1}`);
  await fixture.command(`data merge block ${d} ${held(H, 64000)}`);
  for (let i = 0; i < 2; i++) { await fixture.stepServer(1); dumped.push(await amount(d)); }
  await fixture.command(`data merge block ${d} ${held(H, 54500)}`);
  await fixture.stepServer(1); dumped.push(await amount(d));
  log.dumped = dumped;
  assert.deepEqual(dumped, [63840, 63680, 63000, 62000, 54400]);
  // Dumping stops the ejector; dumping excess doesn't.
  const e = at(18), f = at(18, 0, -1);
  await fixture.command(`setblock ${e} ${tank('basic')}`);
  await fixture.command(`setblock ${f} ${tank('basic')}`);
  await fixture.command(`data merge block ${e} {dumping:2}`);
  await fixture.command(`data merge block ${e} ${held(H, 30000)}`);
  await fixture.stepServer(1);
  assert.deepEqual([await amount(e), await amount(f)], [29840, 0], 'dumping: no eject');
  await fixture.command(`data merge block ${e} {dumping:1}`);
  await fixture.command(`data merge block ${e} ${held(H, 64000)}`);
  await fixture.stepServer(1);
  assert.deepEqual([await amount(e), await amount(f)], [62000, 1000], 'dumping excess: eject, then 1000 dumped');
  fixture.mark('dumping 160 a tick and no eject; dumping excess 1000 a tick to 54 400, ejecting');

  // The creative tank holding 5 mB gives 5 a tick and keeps them.
  const c = at(21), g = at(21, 0, -1);
  await fixture.command(`setblock ${c} ${tank('creative')}`);
  await fixture.command(`setblock ${g} ${tank('basic')}`);
  await fixture.command(`data merge block ${c} ${held(H, 5)}`);
  await fixture.stepServer(2);
  assert.deepEqual([await amount(c), await amount(g)], [5, 10]);
  fixture.mark('a creative tank of 5 mB gives 5 a tick and keeps them');

  // The drain and fill slots.
  const dr = at(24);
  await fixture.command(`setblock ${dr} ${tank('basic')}`);
  await fixture.command(`data merge block ${dr} {component_config:{config2:[I;0,0,0,0,0,0],eject2:0b}}`);
  await fixture.command(`data merge block ${dr} ${held(H, 30000)}`);
  await fixture.command(`data merge block ${dr} {Items:[${item(0, 'basic')}]}`);
  await fixture.stepServer(2);
  assert.deepEqual([await amount(dr), await slotHeld(dr, 0)], [28000, 2000], 'the drain slot fills 1000 a tick');
  const fl = at(26);
  await fixture.command(`setblock ${fl} ${tank('basic')}`);
  await fixture.command(`data merge block ${fl} {component_config:{config2:[I;0,0,0,0,0,0],eject2:0b}}`);
  await fixture.command(`data merge block ${fl} {Items:[${item(1, 'basic', H, 5000)}]}`);
  await fixture.stepServer(2);
  assert.deepEqual([await amount(fl), await slotHeld(fl, 1)], [2000, 3000], 'the fill slot gives 1000 a tick');
  fixture.mark('the drain slot fills a tank item 1000 a tick, the fill slot empties one 1000 a tick');

  // The separator's left output slot fills a tank item; full, the item ejector puts it in a chest on its left (east).
  const sep = at(28), chest = at(29);
  await fixture.command(`setblock ${sep} mekanism:electrolytic_separator`);
  await fixture.command(`setblock ${chest} minecraft:chest`);
  await fixture.command(`data merge block ${sep} {chemical_tanks:[{tank:0b,stored:{id:"${H}",amount:2400L}}],component_config:{eject2:0b}}`);
  await fixture.command(`data merge block ${sep} {Items:[${item(1, 'basic')}]}`);
  const fills: number[][] = [];
  // The separator's tanks by `/mek probe`: its compound's items keep chemical tanks of their own.
  const leftTank = async () => (await mek(`mek probe ${sep}`)).tanks[0].amount;
  for (let i = 0; i < 3; i++) { await fixture.stepServer(1); fills.push([await leftTank(), await slotHeld(sep, 1)]); }
  log.separator_slot = fills;
  assert.deepEqual(fills, [[1400, 1000], [400, 2000], [0, 2400]]);
  await fixture.command(`data merge block ${sep} {chemical_tanks:[{tank:0b,stored:{id:"${H}",amount:100L}}],Items:[${item(1, 'basic', H, 63950)}]}`);
  await fixture.command(`data merge block ${sep} {component_config:{config3:[I;1,5,6,8,0,0],eject3:1b}}`);
  await fixture.stepServer(1);
  assert.equal(await leftTank(), 50, 'the item takes the 50 it has room for');
  let ticks = 1;
  while (!(await fixture.command(`data get block ${chest} Items`)).chat?.join(' ').includes('basic_chemical_tank') && ticks < 13) {
    await fixture.stepServer(1); ticks++;
  }
  const inChest = (await fixture.command(`data get block ${chest} Items`)).chat?.join(' ') ?? '';
  log.chest = { ticks, inChest };
  assert(/amount: ?64000L/.test(inChest), `the full item in the chest: ${inChest}`);
  fixture.mark('the separator\'s output slot fills a tank item 1000 a tick; full, the item ejector puts it in a chest');

  // The infuser's infusion slot takes 1000 carbon from a tank item.
  const inf = at(31);
  await fixture.command(`setblock ${inf} mekanism:metallurgic_infuser`);
  await fixture.command(`data merge block ${inf} {Items:[${item(0, 'basic', 'mekanism:carbon', 5000)}]}`);
  await fixture.stepServer(1);
  const infused = (await mek(`mek probe ${inf}`)).tanks[0];
  assert.deepEqual([infused.chemical, infused.amount], ['mekanism:carbon', 1000]);
  assert.equal(await slotHeld(inf, 0), 4000);
  fixture.mark('the infuser\'s infusion slot takes 1000 carbon a tick from a tank item');
  await fixture.hold(false);

  // Its item: placed from the east holding 12 345 mB, it faces east and holds them; its drop keeps them.
  const placedAt = at(2, 0, 5);
  await fixture.replace('hotbar.0', `${tank('basic')}[minecraft:custom_data={mekanism:${held(H, 12345)}}]`);
  await fixture.select(0, tank('basic'));
  await fixture.command(`tp @s ${x + 4.5} ${y} ${z + 5.5}`);
  await fixture.raw('look 90 30');
  await until(() => fixture.command('data get entity @s Rotation[0]'), r => /90(\.0+)?f/.test((r.chat ?? []).join(' ')), 'the server sees yaw 90');
  await fixture.raw(`interact ${x + 2} ${y - 1} ${z + 5} up`);
  const placed = await until(() => saved(placedAt).catch(() => ''), v => v.includes('basic_chemical_tank'), 'the tank is placed');
  log.placed = placed;
  assert.deepEqual(tankIn(placed), [H, 12345]);
  assert(/facing: ?["']east["']/.test(placed), `faces east: ${placed}`);
  await fixture.command(`data merge block ${placedAt} {dumping:1}`);
  await fixture.command('gamemode survival');
  await fixture.replace('hotbar.1', 'minecraft:diamond_pickaxe');
  await fixture.select(1, 'minecraft:diamond_pickaxe');
  await fixture.command(`tp @s ${x + 5.5} ${y} ${z + 5.5}`);
  await fixture.raw(`break ${placedAt}`);
  const dropped = async () => (await fixture.command(`execute positioned ${placedAt} run data get entity @e[type=minecraft:item,limit=1,sort=nearest,distance=..3] Item`)).chat?.join(' ') ?? '';
  const drop = await until(dropped, v => v.includes('basic_chemical_tank'), 'the tank drops');
  log.drop = drop;
  assert(/amount: ?12345L/.test(drop) && /dumping: ?1/.test(drop), `the drop keeps its contents and dump mode: ${drop}`);
  await fixture.command('kill @e[type=minecraft:item,distance=..32]');
  const plainAt = at(2, 0, 7);
  await fixture.command(`setblock ${plainAt} ${tank('basic')}`);
  await until(() => fixture.raw(`block ${plainAt}`), v => v.block === tank('basic'), 'the untouched tank reaches the client');
  await fixture.raw(`break ${plainAt}`);
  const plain = await until(async () => (await fixture.command(`execute positioned ${plainAt} run data get entity @e[type=minecraft:item,limit=1,sort=nearest,distance=..3] Item`)).chat?.join(' ') ?? '', v => v.includes('basic_chemical_tank'), 'the untouched tank drops');
  assert(!plain.includes('custom_data'), `an untouched tank drops plain: ${plain}`);
  await fixture.command('kill @e[type=minecraft:item,distance=..32]');
  await fixture.command('gamemode creative');
  fixture.mark('a tank item holding 12 345 mB places a tank facing the player that holds them; its drop keeps them');

  // Its screen: the two slots and the dump mode button.
  const sc = at(5, 0, 5);
  await fixture.command(`setblock ${sc} ${tank('basic')}`);
  await fixture.command(`tp @s ${x + 5.5} ${y} ${z + 7.5}`);
  await until(() => fixture.raw(`block ${sc}`), v => v.block === tank('basic'), 'the tank reaches the client');
  await fixture.raw(`interact ${sc} south`);
  const view = await until(() => fixture.raw('screen'), v => v.screen === 'container' && (v.slots ?? []).some((s: any) => s.slot === 300), 'the tank screen opens');
  const own = (view.slots as any[]).filter(s => s.slot >= 300 && s.slot < 400).map(s => s.slot - 300);
  log.screen_slots = own;
  assert.deepEqual(own, [0, 1], 'the drain and fill slots');
  const scale = Math.max(1, Math.min(Math.floor(960 / 320), Math.floor(540 / 240)));
  const [ox, oy] = [Math.trunc((960 / scale - 176) / 2), Math.trunc((540 / scale - 166) / 2)];
  await Bun.sleep(1500);
  const shot = join(fixture.run, 'chemical-tank.png');
  await fixture.raw(`screenshot ${shot}`);
  log.screenshot = shot;
  await fixture.raw(`click-at ${ox + 164} ${oy + 77}`);
  await until(() => saved(sc), v => /dumping: ?1[,}]/.test(v), 'the dump mode button: idle to dumping excess');
  await fixture.raw('close');
  fixture.mark('its screen shows its two slots; the dump mode button cycles the mode');

  fixture.assertHidden(await fixture.raw('status'));
  console.log('MEK tank', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('MEK tank', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
