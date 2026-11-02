/** The real book, from the player's ATM10 instance (`ATM10_DIR`; this game has no copy): it opens with every chapter
 * and quest; a vanilla item task and a checkmark task complete from the book's own files; an item reward is claimed;
 * quests of mods that aren't carried yet show placeholders; progress stays over a restart. Which quests it uses comes
 * from the files, through what the screen shows. Screenshots go to FTBQ_SHOTS. */
import { existsSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { Book, assert, makeFixture, until, type View } from './common';

const atm10 = process.env.ATM10_DIR;
if (!atm10 || !existsSync(join(atm10, 'config/ftbquests/quests/data.snbt'))) {
  console.log('book-atm10 skipped: no ATM10_DIR');
  process.exit(0);
}
const fixture = makeFixture('ftbquests-atm10');
const book = new Book(fixture);
const vanilla = (id?: string) => !!id && id.startsWith('minecraft:');

type Shown = View['shown'][number];

/** Goes through the chapters until `pick` finds a quest among those drawn. */
async function find(pick: (quest: Shown) => boolean, view: View): Promise<{ quest: Shown; view: View }> {
  for (let index = 0; index < view.chapters; index++) {
    const rows = view.hits.filter(h => h.kind === 'chapter');
    const row = rows.find(h => Number(h.id) === index);
    if (!row) continue;
    await book.click(row);
    await Bun.sleep(250);
    const now = await book.view();
    const quest = now.shown.find(pick);
    if (quest) return { quest, view: now };
    view = now;
  }
  throw new Error('no such quest in the chapters that fit the list');
}

try {
  await fixture.start();
  await fixture.replace('hotbar.0', 'ftbquests:book');
  await fixture.select(0, 'ftbquests:book');
  await fixture.raw('use-item');
  let view = await book.waitOpen();
  console.log(`chapters ${view.chapters}, quests ${view.quests}, first ${view.chapter_file}`);
  // As many chapters as the instance has files; the parser's counts are checked against the files in the crate's tests.
  assert.equal(view.chapters, readdirSync(join(atm10, 'config/ftbquests/quests/chapters')).filter(f => f.endsWith('.snbt')).length);
  assert(view.quests > 0);
  const shots = [await book.shot('atm10-1-open')];
  fixture.mark('the book opens on ATM10\'s files');

  // The zoomed-out graph, and zoomed in.
  const [w, h] = view.size;
  await fixture.raw(`scroll-at ${w - 30} ${h - 12} -2`);
  view = await until(() => book.view(), v => v.zoom === 8, 'zoomed out');
  shots.push(await book.shot('atm10-2-zoomed-out'));
  await fixture.raw(`scroll-at ${w - 30} ${h - 12} 4`);
  view = await until(() => book.view(), v => v.zoom === 24, 'zoomed in');
  shots.push(await book.shot('atm10-3-zoomed-in'));
  await fixture.raw(`scroll-at ${w - 30} ${h - 12} -2`);
  view = await until(() => book.view(), v => v.zoom === 16, 'back');

  // A quest that is open to start, asks for one vanilla item and gives a vanilla item.
  const itemQuest = (q: Shown) => q.deps_done && q.tasks.length === 1 && q.tasks[0].type === 'item' && vanilla(q.tasks[0].detail?.item)
    && q.rewards.some(r => r.type === 'item' && vanilla(r.detail?.item));
  const first = await find(itemQuest, view);
  view = first.view;
  const task = first.quest.tasks[0];
  const reward = first.quest.rewards.find(r => r.type === 'item' && vanilla(r.detail?.item))!;
  console.log(`item quest ${first.quest.id}: ${task.detail.count}x ${task.detail.item}, reward ${reward.detail.count}x ${reward.detail.item}`);
  await book.openQuest(first.quest.id);
  shots.push(await book.shot('atm10-4-quest'));
  const had = await book.count(reward.detail.item);
  await fixture.command(`give @s ${task.detail.item} ${task.detail.count}`);
  // ATM10's detection delay is 60 ticks.
  await until(() => book.status(first.quest.id), s => s.completed, 'the item quest completes');
  fixture.mark('an item task from the files completes');
  await until(() => book.view(), v => v.hits.some(h => h.kind === 'reward' && h.id === reward.id), 'its reward shows');
  shots.push(await book.shot('atm10-5-complete'));
  await book.click(book.hit(await book.view(), 'reward', reward.id));
  await until(() => book.status(reward.id), s => s.claimed, 'the reward is claimed');
  const got = await book.count(reward.detail.item);
  assert(got >= had + reward.detail.count, `${reward.detail.item}: had ${had}, now ${got}, reward ${reward.detail.count}`);
  await book.closeQuest();
  fixture.mark('an item reward from the files is claimed');

  // A checkmark task from the files.
  const checkQuest = (q: Shown) => q.deps_done && !q.completed && q.tasks.some(t => t.type === 'checkmark' && !t.done);
  view = await book.view();
  const second = await find(checkQuest, view);
  const check = second.quest.tasks.find(t => t.type === 'checkmark' && !t.done)!;
  console.log(`checkmark quest ${second.quest.id} in ${second.view.chapter_file}`);
  await book.openQuest(second.quest.id);
  await book.click(book.hit(await book.view(), 'task', check.id));
  await until(() => book.status(check.id), s => s.completed, 'the checkmark completes');
  shots.push(await book.shot('atm10-6-checkmark'));
  await book.closeQuest();
  fixture.mark('a checkmark task from the files completes');

  // A chapter of a mod that isn't carried: placeholders, and nothing falls over.
  const others = (await book.view()).hits.filter(h => h.kind === 'chapter');
  await book.click(others[Math.min(others.length - 1, 12)]);
  await Bun.sleep(400);
  shots.push(await book.shot('atm10-7-other-chapter'));
  const foreign = (q: Shown) => q.tasks.some(t => t.type === 'item' && !vanilla(t.detail?.item));
  view = await book.view();
  const third = await find(foreign, view);
  await book.openQuest(third.quest.id);
  shots.push(await book.shot('atm10-8-placeholder'));
  await book.closeQuest();

  // Over a restart.
  await fixture.raw('key escape tap');
  book.forget();
  await fixture.restart();
  await fixture.raw('command ftbquests open_book');
  view = await book.waitOpen();
  assert.equal((await book.status(first.quest.id)).completed, true);
  assert.equal((await book.status(reward.id)).claimed, true);
  assert.equal((await book.status(check.id)).completed, true);
  fixture.mark('progress stays over a restart');
  console.log(JSON.stringify({ shots }));
} finally {
  await fixture.close();
}
console.log('book-atm10 passed');
