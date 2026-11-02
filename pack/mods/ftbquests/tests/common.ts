/** What the quest book's world tests share: the game with the mod's content, its screen read through `/ftbq_view`,
 * and real clicks at the places the screen says it drew. Run by `pack/scripts/ftbquests.ts world`. */
import assert from 'node:assert/strict';
import { existsSync, readFileSync, rmSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { WorldFixture, until } from '../../../../ae2/tests/world/harness';
import { root } from '../../../../ae2/scripts/common';

export { assert, until };

export const screenshots = process.env.FTBQ_SHOTS ?? 'C:/horizon-work/shots-quests';

export function makeFixture(label: string): WorldFixture {
  const base = resolve(root, '..');
  // The screen writes what it shows here, for `Book.view`.
  process.env.FTBQ_VIEW_FILE = join(process.env.TEMP ?? process.env.TMPDIR ?? '/tmp', `ftbq-view-${process.pid}.json`);
  const content = resolve(process.env.FTBQ_CONTENT ?? 'C:/horizon-work/content-quests');
  const carry = resolve(process.env.FTBQ_CARRY ?? join(base, 'ports/atm10/mods/ftbquests/carry'));
  return new WorldFixture(label, 'server-thread', { devMods: [carry, content] });
}

export type Hit = { kind: string; id: string; rect: [number, number, number, number] };
export type View = {
  open: boolean; size: [number, number]; chapter: number; chapters: number; quests: number; chapter_file?: string;
  zoom?: number; scroll?: [number, number]; quest: string | null; hits: Hit[];
  shown: { id: string; completed: boolean; startable: boolean; deps_done: boolean; dependencies: string[];
    tasks: { id: string; type: string; detail: any; done: boolean }[];
    rewards: { id: string; type: string; detail: any; claimed: boolean }[] }[];
};

/** A book on screen, driven as a player does. */
export class Book {
  constructor(readonly fixture: WorldFixture) {}
  async view(): Promise<View> {
    const file = process.env.FTBQ_VIEW_FILE!;
    if (!existsSync(file)) return { open: false, hits: [], shown: [] } as unknown as View;
    return JSON.parse(readFileSync(file, 'utf8'));
  }
  /** Before a restart: what the last run drew isn't what the next one shows. */
  forget(): void {
    rmSync(process.env.FTBQ_VIEW_FILE!, { force: true });
  }
  async screen(): Promise<string> {
    return (await this.fixture.raw('status')).screen;
  }
  async waitOpen(): Promise<View> {
    await until(() => this.fixture.raw('status'), status => status.screen === 'mod', 'the book opens');
    // A frame has to draw before there is anything to click.
    return until(() => this.view(), view => view.open && view.hits.length > 0, 'the book draws');
  }
  hit(view: View, kind: string, id?: string): Hit {
    const found = view.hits.find(h => h.kind === kind && (id === undefined || h.id === id));
    assert(found, `no ${kind} ${id ?? ''} on the screen: ${JSON.stringify(view.hits.slice(0, 40))}`);
    return found;
  }
  async click(hit: Hit, right = false): Promise<void> {
    const [x, y, w, h] = hit.rect;
    await this.fixture.raw(`click-at ${x + w / 2} ${y + h / 2}${right ? ' right' : ''}`);
    await Bun.sleep(150);
  }
  /** A quest's place on the screen, recentring the graph if it is out of view. */
  async openQuest(id: string): Promise<View> {
    const view = await this.view();
    await this.click(this.hit(view, 'quest', id));
    return until(() => this.view(), v => v.quest === id && v.hits.some(h => h.kind === 'task' || h.kind === 'reward'), `quest ${id} opens`);
  }
  async closeQuest(): Promise<void> {
    await this.fixture.raw('key escape tap');
    await until(() => this.view(), v => v.quest === null, 'the quest panel closes');
  }
  async status(id: string): Promise<{ count: number; started: boolean; completed: boolean; claimed: boolean }> {
    const reply = await this.fixture.raw(`command ftbquests status ${id}`);
    const line = (reply.chat ?? []).find((text: string) => text.startsWith('ftbquests '));
    assert(line, `no status: ${JSON.stringify(reply.chat)}`);
    const m = line.match(/count=(\d+) started=(\w+) completed=(\w+) claimed=(\w+)/);
    assert(m, line);
    return { count: Number(m![1]), started: m![2] === 'true', completed: m![3] === 'true', claimed: m![4] === 'true' };
  }
  /** How many of an item the player has (`/clear` with a count of 0 only counts). */
  async count(item: string): Promise<number> {
    const reply = await this.fixture.raw(`command clear @s ${item} 0`);
    const line = (reply.chat ?? []).join(' ');
    const m = line.match(/Found (\d+)/);
    return m ? Number(m[1]) : 0;
  }
  async shot(name: string): Promise<string> {
    await Bun.sleep(800);
    const path = join(screenshots, `${name}.png`);
    await this.fixture.raw(`screenshot ${path}`);
    return path;
  }
}
