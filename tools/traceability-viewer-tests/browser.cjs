const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const {pathToFileURL} = require('node:url');
const {execFileSync} = require('node:child_process');
const {chromium} = require('playwright');

const repository = process.argv[2] || path.resolve(__dirname, '../..');
const fixtures = fs.mkdtempSync(path.join(os.tmpdir(), 'traceability-browser-'));
const errors = [];
let browser;
async function open(name, viewport = {width: 1512, height: 1050}) {
  const page = await browser.newPage({viewport});
  page.on('pageerror', error => errors.push(error.message));
  page.setDefaultTimeout(5000);
  await page.goto(pathToFileURL(path.join(fixtures, `${name}.html`)).href);
  return page;
}
async function selectComponent(page, name) {
  await page.locator('#components').getByRole('button', {name:`Select component ${name}`,exact:true}).click();
}
async function selectBoundary(page, from, to) {
  await page.locator('#graph').getByRole('button', {name: new RegExp(`^${from} → ${to}:`)}).click();
}
async function selectContract(page, name) {
  await page.locator('#contracts button').filter({hasText: name}).click();
}
async function scenario(name, run) {
  await run();
  console.log(`PASS ${name}`);
}
(async () => {
  execFileSync('python3', [path.join(__dirname, 'fixtures.py'), repository, fixtures], {stdio: 'pipe'});
  browser = await chromium.launch({headless: true});
  await scenario('component → boundary → API → explicit target test → captured source', async () => {
    const page = await open('graph');
    await selectComponent(page, 'app');
    await selectBoundary(page, 'app', 'domain');
    await selectContract(page, 'Boundary::apply');
    const detail = await page.locator('#selection-detail').innerText();
    assert.match(detail, /declared_boundary_contract/);
    assert.doesNotMatch(detail, /concrete_implementation_only/);
    assert.doesNotMatch(detail, /setup_only_untargeted/);
    const test = page.locator('#selection-detail button').filter({hasText:'declared_boundary_contract'});
    await test.focus();
    await page.keyboard.press('Enter');
    assert.equal(await page.locator('#source-panel').evaluate(e => e.open), true);
    assert.match(await page.locator('#source-title').innerText(), /tests\/domain.rs:10/);
    assert.match(await page.locator('#source-code .highlight').innerText(), /captured line 10/);
    assert.equal(await page.evaluate(() => globalThis.sourceExecuted), undefined);
    assert.match(await page.locator('#github').getAttribute('href'), /^https:\/\/github.com\/example\/fixture\/blob\/[a-f0-9]+\/tests\/domain.rs#L10$/);
    assert.equal(await page.locator('#source-code .highlight').isVisible(), true);
    await page.keyboard.press('Escape');
    await page.locator('#source-panel').waitFor({state:'hidden'});
    assert.equal(await page.locator('#source-panel').evaluate(e => e.open), false);
    assert.equal(await test.evaluate(e => e === document.activeElement), true);
    assert.equal(await page.locator('#graph').getByRole('button', {name:/^store → domain:/}).count(), 0);
    assert.equal(await page.locator('#graph').getByRole('button', {name:/^app → app:/}).count(), 0);
    await page.close();
  });
  await scenario('closing source preserves focus moved before the queued close event', async () => {
    const page = await open('graph');
    await page.locator('#source-search').evaluate(e => { e.open = true; });
    await page.locator('#search').fill('captured line 12');
    await page.locator('#source-results button').first().click();
    const focus = await page.evaluate(async () => {
      const panel = document.getElementById('source-panel');
      const closed = new Promise(resolve => panel.addEventListener('close', resolve, {once: true}));
      panel.close();
      document.getElementById('search').focus();
      await closed;
      return document.activeElement.id;
    });
    assert.equal(focus, 'search');
    await page.locator('#search').fill('');
    await selectComponent(page, 'app');
    await selectBoundary(page, 'app', 'domain');
    await page.close();
  });
  await scenario('no-target filter and query reconcile selection, including zero matches', async () => {
    const page = await open('graph');
    await selectComponent(page, 'app');
    await selectBoundary(page, 'app', 'domain');
    await selectContract(page, 'Boundary::apply');
    await page.locator('#gaps').click();
    assert.equal(await page.locator('#contracts button').filter({hasText:'Boundary::apply'}).count(), 0);
    assert.doesNotMatch(await page.locator('#selection-detail').innerText(), /declared_boundary_contract/);
    await page.locator('#gaps').click();
    await page.locator('#search').fill('declared_boundary_contract');
    assert.match(await page.locator('#selection-title').innerText(), /apply/);
    await page.locator('#search').fill('nothing_matches_this_query');
    assert.equal(await page.locator('#contracts button').count(), 0);
    assert.doesNotMatch(await page.locator('#selection-detail').innerText(), /declared_boundary_contract/);
    assert.equal(await page.locator('#graph').getByRole('button', {name:/ → /}).count(), 0);
    await page.locator('#search').fill('');
    assert.ok(await page.locator('#contracts button').count() > 0);
    await page.close();
  });
  await scenario('all neighbors, keyboard graph selection, all APIs and untargeted test catalog', async () => {
    const page = await open('graph');
    await selectComponent(page, 'app');
    await page.locator('#neighbors').click();
    assert.equal(await page.locator('#graph').getByRole('button', {name:/^adapter-\d → app:/}).count(), 6);
    const boundary = page.locator('#graph').getByRole('button', {name:/^adapter-5 → app:/});
    await boundary.focus();
    await page.keyboard.press('Space');
    assert.match(await page.locator('#selection-title').innerText(), /run/);
    await page.locator('#catalog > summary').click();
    await page.locator('#component').selectOption('');
    await page.locator('#test-kind').selectOption('untargeted');
    assert.match(await page.locator('#tests').innerText(), /setup_only_untargeted/);
    assert.doesNotMatch(await page.locator('#tests').innerText(), /declared_boundary_contract/);
    await page.locator('#test-kind').selectOption('all');
    assert.match(await page.locator('#apis').innerText(), /Engine::apply/);
    await page.locator('#apis').getByRole('button', {name:'domain::Engine::apply',exact:true}).focus();
    await page.keyboard.press('Enter');
    assert.equal(await page.locator('#contracts button[aria-pressed=true]').evaluate(e => e === document.activeElement), true);
    assert.match(await page.locator('#selection-detail').innerText(), /concrete_implementation_only/);
    assert.doesNotMatch(await page.locator('#selection-detail').innerText(), /declared_boundary_contract/);
    await page.close();
  });
  await scenario('captured source search, unresolved calls, references, and compilation metadata remain accessible', async () => {
    const page = await open('graph');
    const search = page.locator('#search');
    await page.locator('#source-search').evaluate(e => { e.open = true; });
    await search.fill('captured line 12');
    assert.match(await page.locator('#source-results').innerText(), /captured line 12/);
    await page.locator('#source-results button').first().click();
    assert.match(await page.locator('#source-code .highlight').innerText(), /captured line 12/);
    await page.keyboard.press('Escape');
    await page.locator('#source-panel').waitFor({state:'hidden'});
    await search.fill('');
    await selectComponent(page, 'app');
    await selectBoundary(page, 'app', 'domain');
    await selectContract(page, 'Boundary::apply');
    assert.match(await page.locator('#selection-detail').textContent(), /store::run/);
    assert.match(await page.locator('#unresolved').textContent(), /function pointer unresolved/);
    assert.match(await page.locator('#configuration').textContent(), /nightly-fixture/);
    assert.match(await page.locator('#configuration').textContent(), /optional-feature/);
    await page.close();
  });
  await scenario('search retains typed spaces and case, and generated source has no fabricated GitHub link', async () => {
    const page = await open('graph');
    await page.locator('#search').pressSequentially('CaPtUrEd line 12');
    assert.equal(await page.locator('#search').inputValue(), 'CaPtUrEd line 12');
    await page.locator('#source-search').evaluate(e => { e.open = true; });
    assert.match(await page.locator('#source-results').innerText(), /captured line 12/);
    await page.locator('#search').fill('');
    await page.locator('#catalog').evaluate(e => { e.open = true; });
    await page.locator('#component').selectOption('domain');
    await page.locator('#apis').getByRole('button', {name:'domain::generated_api',exact:true}).click();
    await page.locator('#selection-detail').getByRole('button', {name:'Open API definition ↗',exact:true}).click();
    assert.match(await page.locator('#source-title').innerText(), /@generated\/domain\/wire.rs:1/);
    assert.equal(await page.locator('#github').isVisible(), false);
    assert.match(await page.locator('#source-status').innerText(), /Generated Rust source/);
    await page.close();
  });
  await scenario('paginated API and evidence lists expose all records on request', async () => {
    const page = await open('bulk');
    assert.match(await page.locator('#selection-title').innerText(), /api_099/);
    assert.equal(await page.locator('#contracts button[aria-pressed=true]').count(), 1);
    await page.locator('#selection-detail .more').click();
    assert.equal(await page.locator('#selection-detail .test-card').count(), 20);
    await page.locator('#catalog').evaluate(e => { e.open = true; });
    while (await page.locator('#apis .more').count()) await page.locator('#apis .more').click();
    assert.equal(await page.locator('#apis .item').count(), 100);
    await page.close();
  });
  await scenario('empty and isolated components remain usable without fabricated boundaries', async () => {
    const empty = await open('empty');
    assert.equal(await empty.locator('#graph').getByRole('button').count(), 0);
    await empty.close();
    const isolated = await open('isolated');
    assert.equal(await isolated.locator('#graph').getByRole('button', {name:/ → /}).count(), 0);
    await isolated.locator('#catalog').evaluate(e => { e.open = true; });
    assert.match(await isolated.locator('#apis').innerText(), /standalone/);
    assert.match(await isolated.locator('#tests').innerText(), /orphan_test/);
    await isolated.close();
  });
  await scenario('mobile page stays within the viewport while the graph scrolls internally', async () => {
    const page = await open('graph', {width:390,height:844});
    const sizes = await page.evaluate(() => ({viewport:innerWidth,document:document.documentElement.scrollWidth}));
    assert.equal(sizes.document, sizes.viewport);
    await selectComponent(page, 'app');
    await selectBoundary(page, 'app', 'domain');
    await selectContract(page, 'Boundary::apply');
    assert.match(await page.locator('#selection-detail').innerText(), /declared_boundary_contract/);
    await page.close();
  });
  assert.deepEqual(errors, []);
  console.log('PASS no browser runtime errors');
})().catch(error => {
  console.error(error);
  process.exitCode = 1;
}).finally(async () => {
  if (browser) await browser.close();
  fs.rmSync(fixtures, {recursive:true,force:true});
});
