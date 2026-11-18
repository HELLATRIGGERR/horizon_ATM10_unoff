/** FTB Quests in a world, as the `ftbquests` native bundle runs it, on ATM10's own quest book: the book read whole
 * (its chapters and quests counted against a scan of the instance's files), the book item and `/ftbquests open_book`
 * opening the book screen, an item task completed from the inventory, a checkmark ticked in the quest view, an item
 * reward claimed there, and all of it after a restart from `<world>/ftbquests/<team uuid>.snbt`. Then a reward of each
 * other kind ATM10's book has (xp, xp_levels, random, loot, choice with its picker, command, toast), each the first of
 * its kind in the book (its quest completed with `/ftbquests_probe complete`), claimed in the quest view or with
 * "take all"; `/ftbquests block_rewards` holding the buttons back; a command reward's permission gate; and after the
 * restart, with the test's copy of the book set to auto-claim, an xp reward claimed as its quest completes.
 * Screenshots of the chapter list with a quest grid, a quest's view, its rewards, the choice picker and the toasts.
 *
 * ATM10's quest folder is copied from the instance (`ATM10_DIR`) into this test's own game folder; nothing of it is
 * in the repository. `pack/scripts/ftbquests.ts world` sets `AE2_TEST_BINARY`, `FTBQ_CONTENT_DIR` (the content mod
 * with the book item, `--dev-mod`) and `FTBTEAMS_DEV=1`. */
import assert from 'node:assert/strict';
import { cpSync, existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
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
/** Completes the quest for the player's team (an operator's `change_progress complete`), whatever its dependencies. */
async function complete(quest: string): Promise<void> {
  await fixture.command(`ftbquests_probe complete ${quest}`);
  await until(server, (p: any) => mine(p).done.includes(quest), `quest ${quest} completed`, 10000);
}
/** The quest's view in the open book. */
async function viewQuest(chapter: string, quest: string): Promise<void> {
  await book(` chapter ${chapter}`);
  await clickHit('quest', quest, `quest ${quest}`);
  await until(book, (b: any) => b.view === quest, `quest ${quest}'s view`, 10000);
}
const claims = (p: any) => mine(p).claims ?? [];
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

  // Slice 2: a reward of each kind, the first of its kind in ATM10's book.
  for (const kind of ['xp', 'xp_levels', 'random', 'loot', 'choice', 'command', 'toast']) assert(pick[kind]?.reward, `a ${kind} reward to claim: ${JSON.stringify(pick[kind])}`);
  const toasts = async () => (await book()).toasts.length;

  // xp, from its quest's view.
  const xp = pick.xp;
  let prev = await server();
  await complete(xp.quest);
  await until(book, (b: any) => b.toasts.some((t: string) => t === 'quest: Quest done'), 'the completion toast', 10000);
  await viewQuest(xp.chapter, xp.quest);
  await shot('rewards');
  await clickHit('claim', xp.reward, 'the xp claim button');
  p = await until(server, (p: any) => p.xp_given >= prev.xp_given + xp.xp, `${xp.xp} xp given`, 10000);
  assert(claims(p).includes(xp.reward), 'the xp reward is claimed');
  assert(p.xp.level > prev.xp.level || p.xp.progress > prev.xp.progress, `the player's experience grows: ${JSON.stringify([prev.xp, p.xp])}`);
  fixture.mark(`xp: ${xp.xp} points claimed (${xp.quest}), level ${p.xp.level}`);

  // xp_levels, with "take all": every waiting reward but the excluded ones and the choices.
  const lv = pick.xp_levels;
  prev = await server();
  await complete(lv.quest);
  await book(` chapter ${lv.chapter}`);
  await clickHit('claim_all', '', 'the take-all button');
  p = await until(server, (p: any) => lv.claim_all.every((r: string) => claims(p).includes(r)), "take all claims the quest's rewards", 10000);
  for (const r of lv.quest_rewards.filter((r: string) => !lv.claim_all.includes(r))) assert(!claims(p).includes(r), `${r} is left to its own button`);
  if (!claims(p).includes(lv.reward)) {
    await viewQuest(lv.chapter, lv.quest);
    await clickHit('claim', lv.reward, 'the xp_levels claim button');
    p = await until(server, (p: any) => claims(p).includes(lv.reward), 'the xp_levels reward is claimed', 10000);
  }
  assert(p.levels_given >= prev.levels_given + lv.levels, `${lv.levels} levels given: ${p.levels_given - prev.levels_given}`);
  fixture.mark(`xp_levels: ${lv.levels} levels (${lv.excluded ? 'excluded from take all, its own button' : 'with take all'}); take all took ${lv.claim_all.length}`);

  // Blocked rewards: no claim button until the team is unblocked; then random, from its table.
  const rnd = pick.random;
  await fixture.command('ftbquests block_rewards true');
  await complete(rnd.quest);
  await viewQuest(rnd.chapter, rnd.quest);
  b = await until(book, (b: any) => b.blocked, 'the book hears the rewards are blocked', 10000);
  assert(!b.hits.some((h: any) => h.kind === 'claim'), 'no claim button while blocked');
  await shot('blocked');
  await fixture.command('ftbquests block_rewards false');
  await until(book, (b: any) => !b.blocked, 'unblocked', 10000);
  await viewQuest(rnd.chapter, rnd.quest);
  let n = await toasts();
  await clickHit('claim', rnd.reward, 'the random claim button');
  p = await until(server, (p: any) => claims(p).includes(rnd.reward), 'the random reward is claimed', 10000);
  b = await until(book, (b: any) => b.toasts.length > n, 'the toast of what it gave', 10000);
  fixture.mark(`random: one draw from table ${rnd.table} (${rnd.entries} rewards) ("${b.toasts.at(-1)}" toast); blocked rewards held the button back`);

  // loot: loot_size draws.
  const lt = pick.loot;
  await complete(lt.quest);
  await viewQuest(lt.chapter, lt.quest);
  await clickHit('claim', lt.reward, 'the loot claim button');
  p = await until(server, (p: any) => claims(p).includes(lt.reward), 'the loot reward is claimed', 10000);
  fixture.mark(`loot: ${lt.loot_size} draw(s) from table ${lt.table}`);

  // choice: the picker, then one of the table's rewards.
  const ch = pick.choice;
  await complete(ch.quest);
  await viewQuest(ch.chapter, ch.quest);
  await clickHit('choose', ch.reward, 'the choice button');
  b = await until(book, (b: any) => b.picking === ch.reward && b.hits.some((h: any) => h.kind === 'pick'), 'the picker opens', 10000);
  await shot('picker');
  await clickHit('pick', `${ch.reward}:0`, "the picker's first reward");
  p = await until(server, (p: any) => claims(p).includes(ch.reward), 'the choice reward is claimed', 10000);
  b = await until(book, (b: any) => b.picking === null && b.claimed.includes(ch.reward), 'the picker closes', 10000);
  fixture.mark(`choice: picked the first of ${ch.entries} from table ${ch.table}`);

  // command: run as the player at its permission_level, on the next tick.
  const cm = pick.command;
  await complete(cm.quest);
  await viewQuest(cm.chapter, cm.quest);
  await clickHit('claim', cm.reward, 'the command claim button');
  p = await until(server, (p: any) => p.ran.some((r: any) => r.reward === cm.reward), 'the command reward runs', 10000);
  const run = p.ran.find((r: any) => r.reward === cm.reward);
  assert.equal(run.level, cm.permission_level);
  assert.equal(run.line, cm.command.replace(/^\//, ''), 'the command as written, its slash off');
  log.command = run;
  // The same path with a vanilla command: @p is the player; a command above the level is refused.
  await fixture.command('ftbquests_probe command 2 give @p minecraft:diamond 3');
  p = await until(server, (p: any) => p.ran.some((r: any) => r.line === 'give @p minecraft:diamond 3'), 'a level 2 command runs', 10000);
  assert(p.ran.find((r: any) => r.line === 'give @p minecraft:diamond 3').ran, `give at level 2: ${JSON.stringify(p.ran)}`);
  await fixture.command('ftbquests_probe command 2 debug stop');
  p = await until(server, (p: any) => p.ran.some((r: any) => r.line === 'debug stop'), 'a level 3 command at level 2', 10000);
  const dbg = p.ran.find((r: any) => r.line === 'debug stop');
  assert(!dbg.ran && dbg.needs === 3, `debug (level 3) is refused at level 2: ${JSON.stringify(dbg)}`);
  assert(p.told.length > 0, 'the commands\' output reaches the chat');
  fixture.mark(`command: "${run.line}" at level ${run.level} ran=${run.ran} (${run.output.join(' / ') || 'no output'}); give @p at 2 ran, debug (3) at 2 refused`);

  // toast: a toast of its own.
  const ts = pick.toast;
  await complete(ts.quest);
  await viewQuest(ts.chapter, ts.quest);
  n = await toasts();
  await clickHit('claim', ts.reward, 'the toast claim button');
  p = await until(server, (p: any) => claims(p).includes(ts.reward), 'the toast reward is claimed', 10000);
  b = await until(book, (b: any) => b.toasts.some((t: string) => t.startsWith('toast: ')), 'its toast shows', 10000);
  await fixture.raw('key escape tap');
  await shot('toasts');
  fixture.mark(`toast: shown ("${b.toasts.find((t: string) => t.startsWith('toast: '))}"); ${b.toasts.length} toasts in all`);
  log.toasts = b.toasts;
  p = await server();
  const done = [...mine(p).done].sort();

  // After the restart the test's copy of the book auto-claims (ATM10's own says `disabled`).
  const data = join(fixture.game, 'config/ftbquests/quests/data.snbt');
  writeFileSync(data, readFileSync(data, 'utf8').replace(/default_autoclaim_rewards: "disabled"/, 'default_autoclaim_rewards: "enabled"'));

  // After a restart: the team's file, the progress, the claims.
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
  assert(claims(p).includes(ch.reward) && claims(p).includes(cm.reward), 'the claims after a restart');
  fixture.mark(`after a restart: ${done.length} objects done, the rewards still taken (${file})`);

  // A quest's rewards after the restart, once the start's toasts are gone: the claimed ones say so.
  await viewQuest(rnd.chapter, rnd.quest);
  await Bun.sleep(6000);
  await shot('rewards-after-restart');

  // Auto-claim: the xp reward of another quest, claimed as the quest completes.
  const au = pick.auto;
  assert(au?.reward, `a second xp quest: ${JSON.stringify(au)}`);
  assert(!mine(await server()).done.includes(au.quest), 'its quest waits');
  prev = await server();
  await complete(au.quest);
  p = await until(server, (p: any) => claims(p).includes(au.reward), 'the auto-claimed reward', 10000);
  assert(p.xp_given >= prev.xp_given + au.xp, `its xp given: ${JSON.stringify([prev.xp_given, prev.xp, p.xp_given, p.xp])}`);
  fixture.mark(`auto-claim: ${au.xp} xp of ${au.quest} claimed as it completed`);
  fixture.assertHidden(await fixture.raw('status'));
  console.log('FTBQ numbers', JSON.stringify(log));
  fixture.report('passed');
} catch (error) {
  console.log('FTBQ numbers', JSON.stringify(log));
  fixture.report('failed', error); throw error;
} finally { await fixture.close(); }
