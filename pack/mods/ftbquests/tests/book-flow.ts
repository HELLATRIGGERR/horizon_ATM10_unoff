/** The quest book's rules from end to end, on a small book of the test's own (no quest data of any pack):
 * - the book item opens the screen, and `/ftbquests open_book` does;
 * - an item task that only looks completes when the player carries the items, and unlocks the next quest;
 * - a checkmark task completes when pressed, only once it is unlocked;
 * - an item task that consumes takes the items when pressed;
 * - an item reward is claimed with its button, once, and the unsupported ones stay as they are;
 * - a kill task shows as unsupported and never completes;
 * - scrolling and zoom move the graph;
 * - progress stays over a save and restart, and rewards aren't claimed twice. */
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { Book, assert, makeFixture, until } from './common';

const fixture = makeFixture('ftbquests-flow');
const quests = join(fixture.game, 'config/ftbquests/quests');
mkdirSync(join(quests, 'chapters'), { recursive: true });
mkdirSync(join(quests, 'lang'), { recursive: true });
writeFileSync(join(quests, 'data.snbt'), `{
\tdefault_quest_shape: "circle"
\tdefault_consume_items: false
\tprogression_mode: "linear"
\tdetection_delay: 0
\ticon: { id: "minecraft:book" }
}\n`);
writeFileSync(join(quests, 'chapter_groups.snbt'), `{
\tchapter_groups: [
\t\t{ id: "00000000000000F1" }
\t]
}\n`);
writeFileSync(join(quests, 'lang/en_us.snbt'), `{
\tfile.0000000000000001.title: "Test Book"
\tchapter_group.00000000000000F1.title: "Group"
\tchapter.00000000000000C1.title: "First"
\tchapter.00000000000000C2.title: "Second"
\tquest.00000000000000A1.title: "Get stone"
\tquest.00000000000000A1.quest_subtitle: "A subtitle"
\tquest.00000000000000A1.quest_desc: ["&aGreen text", "{@pagebreak}", "A second page"]
\tquest.00000000000000A2.title: "Press it"
\tquest.00000000000000A3.title: "Hand in dirt"
\tquest.00000000000000A4.title: "Kill"
}\n`);
writeFileSync(join(quests, 'chapters/first.snbt'), `{
\tid: "00000000000000C1"
\tfilename: "first"
\tgroup: ""
\torder_index: 0
\tdefault_quest_shape: ""
\tdefault_hide_dependency_lines: false
\ticon: { id: "minecraft:stone" }
\timages: [ ]
\tquest_links: [ ]
\tquests: [
\t\t{
\t\t\tid: "00000000000000A1"
\t\t\tx: 0.0d
\t\t\ty: 0.0d
\t\t\ttasks: [{ id: "00000000000000B1", type: "item", item: { id: "minecraft:stone", count: 1 }, count: 2L }]
\t\t\trewards: [
\t\t\t\t{ id: "00000000000000D1", type: "item", item: { id: "minecraft:torch", count: 1 }, count: 4 }
\t\t\t\t{ id: "00000000000000D2", type: "xp", xp: 100 }
\t\t\t]
\t\t}
\t\t{
\t\t\tid: "00000000000000A2"
\t\t\tx: 2.0d
\t\t\ty: 0.0d
\t\t\tshape: "hexagon"
\t\t\tdependencies: ["00000000000000A1"]
\t\t\ttasks: [{ id: "00000000000000B2", type: "checkmark" }]
\t\t\trewards: [{ id: "00000000000000D3", type: "item", item: { id: "minecraft:diamond", count: 1 } }]
\t\t}
\t\t{
\t\t\tid: "00000000000000A3"
\t\t\tx: 4.0d
\t\t\ty: 0.0d
\t\t\tshape: "gear"
\t\t\tsize: 2.0d
\t\t\tdependencies: ["00000000000000A2"]
\t\t\ttasks: [{ id: "00000000000000B3", type: "item", item: { id: "minecraft:dirt", count: 1 }, count: 3L, consume_items: true }]
\t\t}
\t\t{
\t\t\tid: "00000000000000A4"
\t\t\tx: 2.0d
\t\t\ty: 2.0d
\t\t\tshape: "diamond"
\t\t\tdependencies: ["00000000000000A1"]
\t\t\ttasks: [{ id: "00000000000000B4", type: "kill", entity: "minecraft:zombie", value: 1L }]
\t\t\trewards: [{ id: "00000000000000D4", type: "item", item: { id: "nosuchmod:thing", count: 1 } }]
\t\t}
\t]
}\n`);
writeFileSync(join(quests, 'chapters/second.snbt'), `{
\tid: "00000000000000C2"
\tfilename: "second"
\tgroup: "00000000000000F1"
\torder_index: 0
\tquests: [
\t\t{
\t\t\tid: "00000000000000A9"
\t\t\tx: 0.0d
\t\t\ty: 0.0d
\t\t\ticon: { id: "nosuchmod:thing" }
\t\t\ttasks: [{ id: "00000000000000B9", type: "checkmark" }]
\t\t}
\t]
}\n`);

writeFileSync(join(fixture.game, 'horizon-mods.txt'), 'key.ftbquests.quests:key.keyboard.j\n');

const book = new Book(fixture);
try {
  await fixture.start();
  await fixture.replace('hotbar.0', 'ftbquests:book');
  await fixture.select(0, 'ftbquests:book');

  // The book item opens the screen, on the client that used it.
  await fixture.raw('use-item');
  let view = await book.waitOpen();
  assert.equal(view.chapters, 2);
  assert.equal(view.quests, 5);
  assert.equal(view.chapter_file, 'first');
  assert.equal(view.zoom, 16);
  const shots = [await book.shot('flow-1-open')];
  fixture.mark('the book item opens the screen');

  // Locked quests show but can't be done: the checkmark does nothing yet.
  const second = view.shown.find(q => q.id === '00000000000000A2')!;
  assert.equal(second.startable, false);
  await book.openQuest('00000000000000A2');
  await book.click(book.hit(await book.view(), 'task', '00000000000000B2'));
  await Bun.sleep(300);
  assert.equal((await book.status('00000000000000B2')).completed, false);
  await book.closeQuest();
  fixture.mark('a locked checkmark does nothing');

  // A reward can't be claimed before its quest is done.
  await book.openQuest('00000000000000A1');
  shots.push(await book.shot('flow-2-quest'));
  await book.click(book.hit(await book.view(), 'reward', '00000000000000D1'));
  await Bun.sleep(300);
  assert.equal(await book.count('minecraft:torch'), 0);

  // An item task that looks: one stone is partway, two complete it and unlock the next.
  await fixture.command('give @s minecraft:stone 1');
  await until(() => book.status('00000000000000B1'), s => s.count === 1, 'one stone counts');
  assert.equal((await book.status('00000000000000A1')).completed, false);
  await fixture.command('give @s minecraft:stone 1');
  await until(() => book.status('00000000000000A1'), s => s.completed, 'two stone complete the quest');
  assert.equal(await book.count('minecraft:stone'), 2, 'a task that looks keeps the items');
  fixture.mark('an item task completes from the inventory');

  // The reward's button claims it, once.
  await until(() => book.view(), v => v.quest === '00000000000000A1', 'the panel stays');
  await book.click(book.hit(await book.view(), 'reward', '00000000000000D1'));
  await until(() => book.count('minecraft:torch'), n => n === 4, 'the reward is given');
  await until(() => book.status('00000000000000D1'), s => s.claimed, 'it is marked claimed');
  await book.click(book.hit(await book.view(), 'reward', '00000000000000D1'));
  await Bun.sleep(400);
  assert.equal(await book.count('minecraft:torch'), 4, 'not given twice');
  // The unsupported reward stays as it is.
  await book.click(book.hit(await book.view(), 'reward', '00000000000000D2'));
  await Bun.sleep(200);
  assert.equal((await book.status('00000000000000D2')).claimed, false);
  shots.push(await book.shot('flow-3-claimed'));
  // The description's pages.
  await book.click(book.hit(await book.view(), 'next'));
  await until(() => book.view(), v => v.hits.some(h => h.kind === 'prev'), 'the next page shows');
  await book.closeQuest();
  fixture.mark('an item reward is claimed once');

  // The checkmark now counts, and its quest's reward waits for its button.
  await book.openQuest('00000000000000A2');
  await book.click(book.hit(await book.view(), 'task', '00000000000000B2'));
  await until(() => book.status('00000000000000A2'), s => s.completed, 'the checkmark completes the quest');
  await book.closeQuest();
  fixture.mark('a checkmark task completes');

  // Collect takes what waits.
  await book.click(book.hit(await book.view(), 'collect'));
  await until(() => book.count('minecraft:diamond'), n => n === 1, 'Collect gives the waiting reward');
  fixture.mark('Collect claims what waits');

  // An item task that consumes: the press takes three of five.
  await fixture.command('give @s minecraft:dirt 5');
  await Bun.sleep(300);
  assert.equal((await book.status('00000000000000B3')).count, 0, 'not until it is pressed');
  await book.openQuest('00000000000000A3');
  await book.click(book.hit(await book.view(), 'task', '00000000000000B3'));
  await until(() => book.status('00000000000000A3'), s => s.completed, 'handing in completes it');
  assert.equal(await book.count('minecraft:dirt'), 2, 'it took three');
  await book.closeQuest();
  fixture.mark('an item task that consumes takes the items');

  // A kill task shows, with a reward of a mod that isn't in the game, and pressing it does nothing.
  await book.openQuest('00000000000000A4');
  shots.push(await book.shot('flow-4-unsupported'));
  await book.click(book.hit(await book.view(), 'task', '00000000000000B4'));
  await book.click(book.hit(await book.view(), 'reward', '00000000000000D4'));
  await Bun.sleep(300);
  assert.equal((await book.status('00000000000000A4')).completed, false);
  await book.closeQuest();
  fixture.mark('an unsupported task stays open');

  // Zoom with the wheel, a drag of the graph, and the other chapter.
  view = await book.view();
  const [w, h] = view.size;
  const emptyX = w - 40, emptyY = h - 20;
  await fixture.raw(`scroll-at ${emptyX} ${emptyY} 1`);
  view = await until(() => book.view(), v => v.zoom === 20, 'the wheel zooms in');
  const before = view.scroll!;
  await fixture.raw(`drag-at ${emptyX} ${emptyY} ${emptyX - 30} ${emptyY - 30}`);
  view = await until(() => book.view(), v => v.scroll![0] !== before[0] || v.scroll![1] !== before[1], 'a drag scrolls');
  assert.equal(view.scroll![0], before[0] + 30);
  await book.click(book.hit(view, 'chapter', '1'));
  view = await until(() => book.view(), v => v.chapter_file === 'second', 'the other chapter opens');
  assert(view.shown.some(q => q.id === '00000000000000A9'));
  shots.push(await book.shot('flow-5-placeholder'));
  fixture.mark('zoom, drag and chapters work');

  // Escape closes the screen.
  await fixture.raw('key escape tap');
  await until(() => book.screen(), s => s === 'playing', 'Escape closes the book');

  // The mod's key, bound in Options > Mods (the file below is what that page writes).
  await fixture.raw('key j tap');
  await until(() => book.screen(), s => s === 'mod', 'the key opens the book');
  await fixture.raw('key escape tap');
  await until(() => book.screen(), s => s === 'playing', 'Escape closes it again');
  fixture.mark('the mod\'s key opens the book');

  // Over a save and restart: all still done, and nothing is given twice.
  const before2 = { torch: await book.count('minecraft:torch'), diamond: await book.count('minecraft:diamond') };
  book.forget();
  await fixture.restart();
  await fixture.raw('command ftbquests open_book');
  view = await book.waitOpen();
  fixture.mark('/ftbquests open_book opens the screen');
  for (const id of ['00000000000000A1', '00000000000000A2', '00000000000000A3']) {
    assert.equal((await book.status(id)).completed, true, `${id} after the restart`);
  }
  assert.equal((await book.status('00000000000000D1')).claimed, true);
  assert.equal((await book.status('00000000000000B1')).count, 2);
  await until(() => book.view(), v => v.shown.find(q => q.id === '00000000000000A1')?.completed === true, 'the client gets its progress');
  await book.openQuest('00000000000000A1');
  await book.click(book.hit(await book.view(), 'reward', '00000000000000D1'));
  await Bun.sleep(400);
  assert.equal(await book.count('minecraft:torch'), before2.torch);
  assert.equal(await book.count('minecraft:diamond'), before2.diamond);
  shots.push(await book.shot('flow-6-after-restart'));
  fixture.mark('progress stays over a restart');

  // The save lies in the world's own folder.
  const saves = join(fixture.game, 'saves');
  const worlds = existsSync(saves) ? readdirSync(saves) : [];
  const files = worlds.flatMap(w => { const d = join(saves, w, 'ftbquests'); return existsSync(d) ? readdirSync(d).map(f => join(d, f)) : []; });
  assert.equal(files.length, 1, `one progress file in the world: ${files}`);
  const text = readFileSync(files[0], 'utf8');
  assert(text.includes('"00000000000000A1":'), 'it names the quest');
  assert(text.includes('task_progress'), 'in FTB Quests\' fields');
  console.log(JSON.stringify({ shots }));
} finally {
  await fixture.close();
}
console.log('book-flow passed');
