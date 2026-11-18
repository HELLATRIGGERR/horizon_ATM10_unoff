/** FTB Quests in a world, as the `ftbquests` native bundle runs it, on ATM10's own quest book: the book read whole
 * (its chapters and quests counted against a scan of the instance's files), the book item and `/ftbquests open_book`
 * opening the book screen, an item task completed from the inventory, a checkmark ticked in the quest view, an item
 * reward claimed there, and all of it after a restart from `<world>/ftbquests/<team uuid>.snbt`. Screenshots of the
 * chapter list with a quest grid, and of a quest's view.
 *
 * ATM10's quest folder is copied from the instance (`ATM10_DIR`) into this test's own game folder; nothing of it is
 * in the repository. `pack/scripts/ftbquests.ts world` sets `AE2_TEST_BINARY`, `FTBQ_CONTENT_DIR` (the content mod
 * with the book item, `--dev-mod`) and `FTBTEAMS_DEV=1`. */
import assert from 'node:assert/strict';
import { cpSync, existsSync, readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';

const instance = resolve(process.env.ATM10_DIR ?? '');
const quests = join(instance, 'config/ftbquests/quests');
assert(existsSync(join(quests, 'chapters')), `ATM10's quest book at ${quests} (set ATM10_DIR)`);
const content = resolve(process.env.FTBQ_CONTENT_DIR ?? '');
assert(existsSync(join(content, 'horizon-mod.toml')), `the content mod at ${content} (pack/scripts/ftbquests.ts content)`);
const fixture = new WorldFixture('ftbq-quests', 'server-thread', { devMods: [content] });
const log: Record<string, unknown> = {};

/** The files' own count, by FTB's layout (a top-level key a tab in, a list element two): no shared parser. */
function scan(): { chapters: number; quests: number } {
  let chapters = 0, count = 0;
  for (const name of readdirSync(join(quests, 'chapters')).filter(f => f.endsWith('.snbt'))) {
    chapters++;
    let key = '';
    for (const line of readFileSync(join(quests, 'chapters', name), 'utf8').split(/\r?\n/)) {
      const top = line.match(/^\t([a-z_]+):/);
      if (top) key = top[1];
      else if (key === 'quests' && line === '\t\t{') count++;
    }
  }
  return { chapters, quests: count };
}
async function probeLine(command: string, prefix: string): Promise<any> {
  const reply = await fixture.command(command);
  const line = (reply.chat ?? []).findLast((text: string) => text.startsWith(prefix));
  assert(line, `${command}: ${JSON.stringify(reply.chat)}`);
  return JSON.parse(line.slice(prefix.length));
}
const server = (extra = '') => probeLine(`ftbquests_probe${extra}`, 'FTBQ ');
const book = (extra = '') => probeLine(`ftbquestsbook${extra}`, 'FTBQB ');
const mine = (p: any) => p.teams.find((t: any) => t.id === p.team) ?? { done: [], progress: {}, claimed: 0 };
async function clickHit(kind: string, id: string, what: string): Promise<void> {
  const b = await until(book, (b: any) => b.hits.some((h: any) => h.kind === kind && h.id === id), `${what}: on the screen`, 10000);
  const r = b.hits.find((h: any) => h.kind === kind && h.id === id).rect;
  await fixture.raw(`click-at ${(r[0] + r[2] / 2).toFixed(1)} ${(r[1] + r[3] / 2).toFixed(1)}`);
}
async function shot(name: string): Promise<void> {
  await Bun.sleep(500);
  const file = join(fixture.run, `ftbquests-${name}.png`);
  await fixture.raw(`screenshot ${file}`);
  ((log.screenshots ??= []) as string[]).push(file);
}

try {
  cpSync(quests, join(fixture.game, 'config/ftbquests/quests'), { recursive: true });
  const files = scan();
  log.scan = files;
  await fixture.start();
  await fixture.command('gamemode creative');

  // The book, read whole.
  let p = await server(' pick');
  assert.equal(p.file_error, null, 'the quest book reads');
  assert.deepEqual([p.chapters, p.quests], [files.chapters, files.quests], 'chapters and quests as the files have them');
  assert(p.quest_dir.startsWith(fixture.game), `the book is the game folder's: ${p.quest_dir}`);
  log.book = { chapters: p.chapters, quests: p.quests, tasks: p.tasks, rewards: p.rewards };
  const pick = p.pick;
  assert(pick.item && pick.check, `an item quest and a checkmark quest to play: ${JSON.stringify(pick)}`);
  log.pick = pick;
  fixture.mark(`the book: ${p.chapters} chapters, ${p.quests} quests, as the instance's files`);

  // The book item opens the book; so does /ftbquests open_book.
  await fixture.replace('hotbar.0', 'ftbquests:book');
  await fixture.select(0, 'ftbquests:book');
  await fixture.raw('use-item');
  let b = await until(book, (b: any) => b.open && b.syncs > 0, 'the book item opens the book', 10000);
  assert(b.drawn > 0, 'the first chapter\'s quests are drawn');
  await shot('chapters');
  fixture.mark(`the book item opens the book (${b.chapter}: ${b.drawn} quests drawn)`);
  await fixture.raw('key escape tap');
  await until(book, (b: any) => !b.open, 'Escape closes the book', 10000);
  await fixture.command('ftbquests open_book');
  await until(book, (b: any) => b.open, '/ftbquests open_book opens the book', 10000);
  fixture.mark('/ftbquests open_book opens the book');

  // An item task, from the inventory.
  const it = pick.item;
  await fixture.command(`give @s ${it.task_item} ${it.task_count}`);
  p = await until(server, (p: any) => mine(p).done.includes(it.quest), `the item quest ${it.quest} completes from the inventory`, 20000);
  assert.equal(mine(p).progress[it.task], it.task_count);
  fixture.mark(`an item task (${it.task_count} ${it.task_item}) completes its quest for the team`);

  // Its reward, claimed in the quest's view.
  await book(` chapter ${it.chapter}`);
  await clickHit('quest', it.quest, 'the item quest');
  b = await until(book, (b: any) => b.view === it.quest, 'a click opens the quest\'s view', 10000);
  await shot('quest-view');
  const before = (await server()).gave;
  await clickHit('claim', it.reward, 'the claim button');
  p = await until(server, (p: any) => p.gave >= before + it.reward_count, 'the reward is given', 10000);
  b = await until(book, (b: any) => b.claimed.includes(it.reward), 'the client hears the claim', 10000);
  assert(!b.hits.some((h: any) => h.kind === 'claim' && h.id === it.reward), 'no second claim');
  fixture.mark(`an item reward (${it.reward_count} ${it.reward_item}) claimed`);

  // A checkmark, ticked in its quest's view.
  const ck = pick.check;
  await fixture.raw('key escape tap');
  await fixture.command('ftbquests open_book');
  await until(book, (b: any) => b.open, 'the book again', 10000);
  await book(` chapter ${ck.chapter}`);
  await clickHit('quest', ck.quest, 'the checkmark quest');
  await clickHit('check', ck.task, 'the checkmark');
  p = await until(server, (p: any) => mine(p).done.includes(ck.task), 'the checkmark is ticked', 10000);
  fixture.mark(`a checkmark task ticked (${ck.quest} ${mine(p).done.includes(ck.quest) ? 'complete' : 'waits on its dependencies'})`);
  const done = [...mine(p).done].sort();

  // After a restart: the team's file, the progress, the claims.
  await fixture.raw('key escape tap');
  const world = p.world, team = p.team;
  await fixture.restart();
  const file = join(world, 'ftbquests', `${team}.snbt`);
  assert(existsSync(file), file);
  const text = readFileSync(file, 'utf8');
  for (const key of ['task_progress: {', 'completed: {', 'claimed_rewards: {', `${it.reward}`]) assert(text.includes(key), `${key} in ${text.slice(0, 300)}`);
  p = await server();
  assert.deepEqual([...mine(p).done].sort(), done, 'the progress after a restart');
  await fixture.command('ftbquests open_book');
  b = await until(book, (b: any) => b.open && b.claimed.includes(it.reward) && b.completed >= done.length, 'the client gets the progress after a restart', 20000);
  await book(` chapter ${it.chapter}`);
  await until(book, (b: any) => b.hits.some((h: any) => h.id === it.quest), 'the chapter is drawn', 10000);
  await shot('after-restart');
  fixture.mark(`after a restart: ${done.length} objects done, the reward still taken (${file})`);
  fixture.assertHidden(await fixture.raw('status'));
  console.log('FTBQ numbers', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('FTBQ numbers', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
