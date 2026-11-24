/** Just Dire Things' tool and armour tiers (ferricore, blazegold, celestigem, eclipse alloy), as the content mod made from the
 * carry of the player's jar makes them: their attack damage, attack speed, armour, toughness and knockback resistance as the
 * mod's `GooTier` and `ArmorTiers` give them. The numbers are the player's attributes with the piece equipped:
 * - a sword in the main hand: the player's 1 plus the tier's damage plus the sword's 3;
 * - a full set of armour: the sum of the pieces' points, the toughness of each piece and the knockback resistance.
 *
 * Needs the staged native build (`AE2_TEST_BINARY`) and the content mod (`JUSTDIRETHINGS_CONTENT_DIR`);
 * `pack/scripts/justdirethings.ts world` sets both. */
import assert from 'node:assert/strict';
import { join, resolve } from 'node:path';
import { WorldFixture } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

const content = resolve(process.env.JUSTDIRETHINGS_CONTENT_DIR ?? join(root, '../.build/content/justdirethings'));
const fixture = new WorldFixture('justdirethings-tools', 'server-thread', { devMods: [content] });
async function attribute(name: string): Promise<number> {
  const reply = await fixture.command(`attribute @s minecraft:${name} get`);
  const line = (reply.chat ?? []).find((text: string) => /value of attribute/.test(text));
  assert(line, `no value for ${name}: ${JSON.stringify(reply.chat)}`);
  return Number(line.match(/ is (-?[\d.]+)\s*$/)![1]);
}
const near = (a: number, b: number) => Math.abs(a - b) < 0.011;

const log: Record<string, unknown> = {};
try {
  await fixture.start();
  await fixture.command('gamemode survival @s');
  // [tier, sword damage, speed, [armour, toughness, knockback resistance] of the whole set]
  const tiers: [string, number, [number, number, number]][] = [
    ['ferricore', 5.5, [15, 0, 0]],
    ['blazegold', 6.0, [15, 0, 0]],
    ['celestigem', 7.0, [20, 8, 0]],
    ['eclipsealloy', 8.0, [20, 12, 0.4]],
  ];
  for (const [tier, sword, [armour, toughness, knockback]] of tiers) {
    await fixture.command(`item replace entity @s weapon.mainhand with justdirethings:${tier}_sword`);
    const damage = await attribute('attack_damage');
    assert(near(damage, 1 + sword), `${tier} sword: attack damage ${damage}, wanted ${1 + sword}`);
    const speed = await attribute('attack_speed');
    assert(near(speed, 4 - 2), `${tier} sword: attack speed ${speed}`);
    await fixture.command('item replace entity @s weapon.mainhand with minecraft:air');
    for (const [piece, slot] of [['helmet', 'head'], ['chestplate', 'chest'], ['leggings', 'legs'], ['boots', 'feet']]) {
      await fixture.command(`item replace entity @s armor.${slot} with justdirethings:${tier}_${piece}`);
    }
    const total = await attribute('armor');
    assert(near(total, armour), `${tier} set: armour ${total}, wanted ${armour}`);
    const tough = await attribute('armor_toughness');
    assert(near(tough, toughness), `${tier} set: toughness ${tough}, wanted ${toughness}`);
    const resist = await attribute('knockback_resistance');
    assert(near(resist, knockback), `${tier} set: knockback resistance ${resist}, wanted ${knockback}`);
    log[tier] = { damage, armour: total, toughness: tough, knockback: resist };
    fixture.mark(`${tier}: sword damage ${damage}, set armour ${total}, toughness ${tough}, knockback resistance ${resist}`);
    for (const slot of ['head', 'chest', 'legs', 'feet']) await fixture.command(`item replace entity @s armor.${slot} with minecraft:air`);
  }
  // Pickaxes: Celestigem's 5 and Eclipse Alloy's 6 points of damage, hoes 2 and 3.
  for (const [item, want] of [['celestigem_pickaxe', 6], ['eclipsealloy_pickaxe', 7], ['celestigem_hoe', 3], ['eclipsealloy_axe', 13], ['celestigem_paxel', 6]] as [string, number][]) {
    await fixture.command(`item replace entity @s weapon.mainhand with justdirethings:${item}`);
    const damage = await attribute('attack_damage');
    assert(near(damage, want), `${item}: attack damage ${damage}, wanted ${want}`);
  }
  fixture.mark('pickaxes, hoes and axes of the two later tiers carry their attack damage');
  fixture.assertHidden(await fixture.raw('status'));
  console.log('JUSTDIRETHINGS tools', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('JUSTDIRETHINGS tools', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
