#!/usr/bin/env node
const { chromium } = require('../tools/traceability-viewer-tests/node_modules/playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const [origin, directory] = process.argv.slice(2);
const actions = [];
const record = (actor, action, value) => {
  actions.push({ actor, action, value });
  fs.writeFileSync(path.join(directory, 'browser-actions.json'), JSON.stringify(actions, null, 2) + '\n');
};
// No traces/HARs: those would retain login cookies and callback query strings.
async function request(page, actor, method, route, body, expected = 200) {
  record(actor, `${method} ${route} request`, { body: body ?? null });
  const result = await page.evaluate(async ({ method, route, body }) => {
    const response = await fetch(route, { method, headers: body === undefined ? {} : { 'Content-Type': 'application/json' }, body: body === undefined ? undefined : JSON.stringify(body) });
    const text = await response.text();
    return { status: response.status, body: text ? JSON.parse(text) : null };
  }, { method, route, body });
  record(actor, `${method} ${route}`, result);
  assert.equal(result.status, expected);
  return result.body;
}
async function connect(page, actor, game) {
  await page.evaluate(game => {
    window.messages = [];
    window.socket = new WebSocket(`${location.protocol === 'https:' ? 'wss:' : 'ws:'}//${location.host}/ws/games/${encodeURIComponent(game)}`, 'shogi.realtime.v1');
    window.socket.onmessage = event => window.messages.push(JSON.parse(event.data));
  }, game);
  return message(page, actor, 'snapshot');
}
async function message(page, actor, type, seq) {
  await page.waitForFunction(({ type, seq }) => window.messages.some(m => m.type === type && (seq === undefined || m.seq === seq)), { type, seq }, { timeout: 15000 });
  const value = await page.evaluate(({ type, seq }) => {
    const i = window.messages.findIndex(m => m.type === type && (seq === undefined || m.seq === seq));
    return window.messages.splice(i, 1)[0];
  }, { type, seq });
  record(actor, `WebSocket ${type}`, value);
  return value;
}
async function send(page, actor, command) {
  record(actor, 'WebSocket send', command);
  await page.evaluate(command => window.socket.send(JSON.stringify(command)), command);
}
async function participation(page, actor, state) {
  let current;
  const deadline = Date.now() + 12000;
  do {
    current = await request(page, actor, 'GET', '/matchmaking/me');
    if (current.state === state) return current.ticket;
    await page.waitForTimeout(100);
  } while (Date.now() < deadline);
  throw new Error(`${actor}: participation did not reach ${state}`);
}
async function main() {
  const browser = await chromium.launch({ headless: true });
  try {
    const contexts = await Promise.all([browser.newContext(), browser.newContext()]);
    const pages = await Promise.all(contexts.map(c => c.newPage()));
    for (let i = 0; i < pages.length; i++) {
      const page = pages[i], actor = ['Alice', 'Bob'][i];
      page.setDefaultTimeout(15000);
      page.on('response', response => {
        const url = new URL(response.url());
        if (url.origin === origin && response.request().resourceType() === 'document') {
          record(actor, 'navigation', { origin, path: url.pathname, status: response.status() });
        }
      });
      await page.goto(origin + '/me');
      await request(page, actor, 'GET', '/me', undefined, 401);
      await request(page, actor, 'GET', '/matchmaking/me', undefined, 401);
      await page.goto(origin + '/auth/google/start');
      assert.equal(new URL(page.url()).origin, origin);
      assert.equal(new URL(page.url()).pathname, '/__verify/authorize');
      record(actor, 'login choice', { origin, path: '/__verify/authorize', actors: ['Alice', 'Bob'] });
      await page.screenshot({ path: path.join(directory, `${actor.toLowerCase()}-login.png`) });
      if (i === 0) {
        const requestId = await page.locator('input[name=request]').inputValue();
        const rejected = await contexts[i].request.post(origin + '/__verify/authorize', {
          headers: { Origin: 'http://foreign.invalid' }, form: { request: requestId, actor: 'alice' }, maxRedirects: 0,
        });
        assert.equal(rejected.status(), 403);
        record(actor, 'foreign-origin actor selection', { status: rejected.status() });
      }
      await page.getByRole('button', { name: actor, exact: true }).click();
      await page.waitForURL(origin + '/me');
      const profile = await request(page, actor, 'GET', '/me');
      assert.equal(profile.display_name, actor);
      const cookies = await contexts[i].cookies(origin);
      const session = cookies.find(c => c.name === 'user_session');
      assert(session && session.httpOnly && session.sameSite === 'Lax' && session.path === '/');
      record(actor, 'ordinary browser session cookie', { present: true, httpOnly: session.httpOnly, sameSite: session.sameSite, path: session.path });
      await request(page, actor, 'PATCH', '/me', { display_name: actor + ' verification' });
      await page.reload();
      const readback = await request(page, actor, 'GET', '/me');
      assert.equal(readback.id, profile.id);
      assert.equal(readback.display_name, actor + ' verification');
      await participation(page, actor, 'idle');
      record(actor, 'authenticated account', { origin, id: profile.id });
      const deniedJoin = await contexts[i].request.put(origin + '/matchmaking/tickets/' + crypto.randomUUID(), {
        headers: { Origin: 'http://foreign.invalid' }, data: { pool: 'ten_minute_30' },
      });
      assert.equal(deniedJoin.status(), 403);
      record(actor, 'foreign-origin gateway join', { status: deniedJoin.status() });
    }
    const profiles = await Promise.all(pages.map((page, i) => request(page, ['Alice', 'Bob'][i], 'GET', '/me')));
    assert.notEqual(profiles[0].id, profiles[1].id);
    const tickets = [crypto.randomUUID(), crypto.randomUUID()];
    for (let i = 0; i < 2; i++) {
      await request(pages[i], ['Alice', 'Bob'][i], 'PUT', '/matchmaking/tickets/' + tickets[i], { pool: 'ten_minute_30' }, 201);
      const heartbeat = await request(pages[i], ['Alice', 'Bob'][i], 'POST', '/matchmaking/tickets/' + tickets[i] + '/heartbeat', { sequence: 1 });
      assert(['waiting', 'matched'].includes(heartbeat.state));
    }
    const matched = await Promise.all(pages.map((page, i) => participation(page, ['Alice', 'Bob'][i], 'matched')));
    assert.equal(matched[0].game_id, matched[1].game_id);
    for (let i = 0; i < 2; i++) await request(pages[i], ['Alice', 'Bob'][i], 'POST', '/matchmaking/tickets/' + tickets[i] + '/ready');
    const playing = await Promise.all(pages.map((page, i) => participation(page, ['Alice', 'Bob'][i], 'playing')));
    const game = playing[0].game_id;
    const deniedSocket = await contexts[0].request.get(origin + '/ws/games/' + encodeURIComponent(game), {
      headers: {
        Origin: 'http://foreign.invalid', Connection: 'Upgrade', Upgrade: 'websocket',
        'Sec-WebSocket-Version': '13', 'Sec-WebSocket-Key': crypto.randomBytes(16).toString('base64'),
        'Sec-WebSocket-Protocol': 'shogi.realtime.v1',
      },
    });
    assert.equal(deniedSocket.status(), 403);
    record('Alice', 'foreign-origin gateway WebSocket', { game, status: deniedSocket.status() });
    const initial = await Promise.all(pages.map((page, i) => connect(page, ['Alice', 'Bob'][i], game)));
    for (const snapshot of initial) {
      assert.equal(snapshot.seq, '0'); assert.equal(snapshot.result, null);
      assert.equal(snapshot.black_user_id, profiles[playing[0].side === 'black' ? 0 : 1].id);
      assert.equal(snapshot.white_user_id, profiles[playing[0].side === 'white' ? 0 : 1].id);
      assert(snapshot.legal_moves.includes('7g7f'));
    }
    const blackIndex = playing[0].side === 'black' ? 0 : 1, whiteIndex = 1 - blackIndex;
    const black = pages[blackIndex], white = pages[whiteIndex];
    const blackName = ['Alice', 'Bob'][blackIndex], whiteName = ['Alice', 'Bob'][whiteIndex];
    const move = { type: 'move', request_id: crypto.randomUUID(), expected_seq: '0', usi: '7g7f' };
    await send(black, blackName, move);
    const accepted = await message(black, blackName, 'accepted', '1');
    assert.equal(accepted.request_id, move.request_id);
    const opponent = await message(white, whiteName, 'event', '1');
    assert.equal(opponent.kind, 'move_committed'); assert.equal(opponent.usi, '7g7f');
    await message(black, blackName, 'event', '1');
    await send(black, blackName, move);
    assert.deepEqual(await message(black, blackName, 'accepted', '1'), accepted);
    await black.evaluate(() => new Promise(resolve => { window.socket.onclose = resolve; window.socket.close(); }));
    const reconnected = await connect(black, blackName, game);
    assert.equal(reconnected.seq, '1'); assert.equal(reconnected.turn, 'white');
    assert.notDeepEqual(reconnected.board, initial[blackIndex].board);
    assert.equal(reconnected.result, null);
    await white.evaluate(() => new Promise(resolve => { window.socket.onclose = resolve; window.socket.close(); }));
    const opponentSnapshot = await connect(white, whiteName, game);
    assert.equal(opponentSnapshot.seq, '1'); assert.deepEqual(opponentSnapshot.board, reconnected.board);
    await send(white, whiteName, { type: 'resign', request_id: crypto.randomUUID(), expected_seq: '1' });
    const resignation = await message(white, whiteName, 'accepted', '2');
    assert.deepEqual(resignation.result, { winner: 'black', reason: 'resignation' });
    for (const [page, name] of [[black, blackName], [white, whiteName]]) {
      const finished = await message(page, name, 'event', '2');
      assert.equal(finished.kind, 'game_finished');
      assert.deepEqual(finished.result, resignation.result);
      await page.evaluate(() => window.socket.close());
      const finalSnapshot = await connect(page, name, game);
      assert.equal(finalSnapshot.seq, '2'); assert.deepEqual(finalSnapshot.result, resignation.result);
      assert.equal(finalSnapshot.legal_moves.length, 0);
      await page.evaluate(() => window.socket.close());
    }
    for (let i = 0; i < 2; i++) {
      const page = pages[i], actor = ['Alice', 'Bob'][i];
      await page.goto(origin + '/me');
      record(actor, 'profile screenshot', { origin, path: '/me' });
      await page.screenshot({ path: path.join(directory, `${actor.toLowerCase()}-result.png`) });
      await request(page, actor, 'PATCH', '/me', { display_name: actor });
      await request(page, actor, 'POST', '/auth/logout', undefined, 204);
      await request(page, actor, 'GET', '/me', undefined, 401);
      await request(page, actor, 'GET', '/matchmaking/me', undefined, 401);
      const denied = await page.evaluate(game => new Promise(resolve => {
        const socket = new WebSocket(`ws://${location.host}/ws/games/${encodeURIComponent(game)}`, 'shogi.realtime.v1');
        socket.onopen = () => { socket.close(); resolve(false); };
        socket.onerror = () => resolve(true);
      }), game);
      assert(denied); record(actor, 'WebSocket after logout', { denied: true });
    }
    record('journey', 'passed', { origin, game, externalGoogle: 'unverified' });
  } finally { await browser.close(); }
}
main().catch(error => {
  // Playwright navigation errors can contain callback query strings. Keep diagnostics sanitized.
  const sanitized = String(error.stack || error).replace(/(https?:\/\/[^\s"']+)\?[^\s"']+/g, '$1?[redacted]');
  record('journey', 'failed', { error: sanitized });
  console.error(sanitized); process.exitCode = 1;
});
