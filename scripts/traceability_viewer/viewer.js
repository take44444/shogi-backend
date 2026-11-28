'use strict';
(() => {
  const snapshot = JSON.parse(document.getElementById('traceability-data').textContent);
  const data = snapshot.bundle;
  const byId = id => document.getElementById(id);
  const apiById = new Map(data.apis.map(api => [api.id, api]));
  const testsByTarget = new Map();
  const apisByComponent = new Map();
  const testsByComponent = new Map();
  const callsByApi = new Map();
  const referencesByApi = new Map();
  const filesByComponent = new Map();
  const boundaries = new Map();
  const boundariesByComponent = new Map();
  const apiSearch = new Map();
  const testSearch = new Map();
  const fileLines = new Map(Object.entries(snapshot.files).map(([path, file]) => [path, file.content.split('\n')]));
  const productionCall = call => call.origins.some(unit => unit.purposes.includes('production') && !unit.is_test && ['lib', 'bin'].includes(unit.target_kind));
  const originLabel = fact => (fact.origins || []).map(unit => `${unit.package} ${unit.target_kind} ${unit.target_name} (${unit.is_test ? 'test harness' : 'normal'}, features: ${unit.features.join(', ') || 'none'}, purposes: ${unit.purposes.join(', ') || 'build script'})`).join(', ');
  const appendIndex = (index, key, value) => {
    if (!index.has(key)) index.set(key, []);
    index.get(key).push(value);
  };
  const addFile = (component, path) => {
    if (!filesByComponent.has(component)) filesByComponent.set(component, new Set());
    filesByComponent.get(component).add(path);
  };
  const apiLabel = id => {
    const api = apiById.get(id);
    return api ? `${api.package} · ${api.name}` : id;
  };
  const shortName = name => name.split('::').slice(-2).join('::');
  for (const api of data.apis) {
    appendIndex(apisByComponent, api.package, api);
    addFile(api.package, api.path);
    callsByApi.set(api.id, {productionCallers: [], productionCalls: [], otherCallers: [], otherCalls: []});
  }
  for (const test of data.tests) {
    const packages = new Set([test.package]);
    for (const id of test.targets) {
      appendIndex(testsByTarget, id, test);
      if (apiById.has(id)) packages.add(apiById.get(id).package);
    }
    for (const component of packages) appendIndex(testsByComponent, component, test);
    addFile(test.package, test.path);
    testSearch.set(test, `${test.name} ${test.package} ${test.path} ${test.targets.map(apiLabel).join(' ')}`.toLowerCase());
  }
  for (const api of data.apis) apiSearch.set(api.id, `${api.name} ${api.package} ${api.path} ${api.kind} ${(testsByTarget.get(api.id) || []).map(test => testSearch.get(test)).join(' ')}`.toLowerCase());
  let productionCallCount = 0;
  for (const call of data.calls) {
    const production = productionCall(call);
    const caller = apiById.get(call.caller);
    const callee = apiById.get(call.callee);
    if (caller) addFile(caller.package, call.path);
    if (callsByApi.has(call.caller)) callsByApi.get(call.caller)[production ? 'productionCalls' : 'otherCalls'].push(call);
    if (callsByApi.has(call.callee)) callsByApi.get(call.callee)[production ? 'productionCallers' : 'otherCallers'].push(call);
    if (!production) continue;
    productionCallCount += 1;
    if (!caller || !callee || caller.package === callee.package) continue;
    const key = JSON.stringify([caller.package, callee.package]);
    if (!boundaries.has(key)) boundaries.set(key, {key, from: caller.package, to: callee.package, apiIds: new Set(), calls: [], callsByTarget: new Map()});
    const boundary = boundaries.get(key);
    boundary.apiIds.add(callee.id);
    boundary.calls.push(call);
    appendIndex(boundary.callsByTarget, callee.id, call);
  }
  for (const boundary of boundaries.values()) {
    boundary.apiIds = [...boundary.apiIds].sort((a, b) => apiById.get(a).name.localeCompare(apiById.get(b).name) || a.localeCompare(b));
    appendIndex(boundariesByComponent, boundary.from, boundary);
    appendIndex(boundariesByComponent, boundary.to, boundary);
  }
  for (const reference of data.references || []) {
    appendIndex(referencesByApi, reference.callee, reference);
    if (apiById.has(reference.caller)) addFile(apiById.get(reference.caller).package, reference.path);
  }
  for (const item of data.unresolved) if (apiById.has(item.caller)) addFile(apiById.get(item.caller).package, item.path);
  for (const unit of data.units) addFile(unit.package, unit.source);
  const state = {componentId: null, boundaryKey: null, apiId: null, query: '', gapsOnly: false, showAllNeighbors: false, testKind: 'all'};
  const pageLimits = new Map();
  let sourceSearchCache = {query: null, componentId: null, matches: []};
  let focusTargets = new Map();
  const node = (tag, value, className) => {
    const element = document.createElement(tag);
    if (value !== undefined) element.textContent = value;
    if (className) element.className = className;
    return element;
  };
  const empty = (element, message) => element.append(node('p', message, 'empty'));
  const rememberFocus = (element, key) => {
    element.dataset.focusKey = key;
    focusTargets.set(key, element);
    return element;
  };
  const action = (label, callback, className, focusKey) => {
    const button = node('button', label, className);
    button.type = 'button';
    button.addEventListener('click', callback);
    if (focusKey) rememberFocus(button, focusKey);
    return button;
  };
  const sourceButton = (location, label, className = 'source-link') => action(label || `${location.path}:${location.line}`, () => showSource(location), className);
  function showSource(location) {
    const file = snapshot.files[location.path];
    const panel = byId('source-panel');
    byId('source-title').textContent = `${location.path}:${location.line}`;
    byId('source-status').textContent = file ? file.status : 'This source file is unavailable in the snapshot.';
    byId('github').hidden = !file || !file.github_url;
    byId('github').removeAttribute('href');
    byId('github-status').textContent = file && file.github_url ? 'Exact revision link available: captured bytes match the committed source.' : 'No exact GitHub revision link is available for this captured source.';
    if (file && file.github_url) byId('github').href = `${file.github_url}#L${location.line}`;
    const code = byId('source-code');
    code.replaceChildren();
    let selected;
    for (const [index, value] of (fileLines.get(location.path) || []).entries()) {
      const line = node('span', undefined, `source-line${index + 1 === location.line ? ' highlight' : ''}`);
      line.append(node('span', String(index + 1), 'line-number'), document.createTextNode(value || ' '));
      code.append(line);
      if (index + 1 === location.line) selected = line;
    }
    if (!panel.open) panel.showModal();
    code.scrollTop = selected ? Math.max(0, selected.offsetTop - code.clientHeight / 3) : 0;
  }
  function renderPaged(parent, records, key, renderRow, batch = 40) {
    const limit = pageLimits.get(key) || batch;
    for (const record of records.slice(0, limit)) parent.append(renderRow(record));
    if (records.length > limit) parent.append(action(`Show more (${records.length - limit} remaining)`, () => {
      pageLimits.set(key, limit + batch);
      parent.replaceChildren();
      renderPaged(parent, records, key, renderRow, batch);
      const next = parent.querySelector('.more');
      if (next) next.focus({preventScroll: true});
      else { parent.tabIndex = -1; parent.focus({preventScroll: true}); }
    }, 'more', `more:${key}`));
  }
  const matchesApi = (id, boundary) => {
    if (state.gapsOnly && (testsByTarget.get(id) || []).length) return false;
    return !state.query || apiSearch.get(id).includes(state.query) || Boolean(boundary && `${boundary.from} ${boundary.to}`.toLowerCase().includes(state.query));
  };
  const visibleApiIds = boundary => boundary.apiIds.filter(id => matchesApi(id, boundary));
  const adjacentBoundaries = () => (boundariesByComponent.get(state.componentId) || []).filter(boundary => visibleApiIds(boundary).length);
  const preferredApi = ids => ids.find(id => (testsByTarget.get(id) || []).length) || ids[0] || null;
  const preferredBoundary = list => [...list].sort((a, b) => Number(b.from === state.componentId) - Number(a.from === state.componentId) || Number(b.apiIds.some(id => (testsByTarget.get(id) || []).length)) - Number(a.apiIds.some(id => (testsByTarget.get(id) || []).length)) || b.apiIds.length - a.apiIds.length || a.key.localeCompare(b.key))[0];
  function reconcile() {
    if (state.boundaryKey) {
      const valid = adjacentBoundaries();
      if (!valid.some(boundary => boundary.key === state.boundaryKey)) state.boundaryKey = preferredBoundary(valid)?.key || null;
    }
    const boundary = boundaries.get(state.boundaryKey);
    const ids = boundary ? visibleApiIds(boundary) : (state.componentId ? apisByComponent.get(state.componentId) || [] : data.apis).filter(api => matchesApi(api.id)).map(api => api.id);
    if (!ids.includes(state.apiId)) state.apiId = preferredApi(ids);
  }
  function chooseComponent(id) {
    state.componentId = id || null;
    state.showAllNeighbors = false;
    state.boundaryKey = preferredBoundary(adjacentBoundaries())?.key || null;
    state.apiId = null;
    render();
  }
  function chooseBoundary(key) {
    state.boundaryKey = key;
    state.apiId = preferredApi(visibleApiIds(boundaries.get(key)));
    render();
  }
  function chooseApi(id, direct = false) {
    if (direct) {
      state.componentId = apiById.get(id).package;
      state.boundaryKey = null;
      state.showAllNeighbors = false;
      if (!matchesApi(id)) { state.query = ''; state.gapsOnly = false; byId('search').value = ''; }
    }
    state.apiId = id;
    render();
    if (direct) focusTargets.get(`api-chip:${id}`).focus({preventScroll: true});
  }
  function renderComponents() {
    byId('components').replaceChildren();
    for (const [index, component] of data.components.entries()) {
      const selected = state.componentId === component.id;
      const button = action(undefined, () => chooseComponent(component.id), `component${selected ? ' selected' : ''}`, `component:${component.id}`);
      button.setAttribute('aria-pressed', String(selected));
      button.setAttribute('aria-label', `Select component ${component.id}`);
      button.append(node('span', component.id), node('small', String(index + 1).padStart(2, '0')));
      byId('components').append(button);
    }
    if (!data.components.length) empty(byId('components'), 'No components captured.');
    byId('component').value = state.componentId || '';
  }
  function renderGraph() {
    const svg = byId('graph');
    svg.replaceChildren();
    const shape = (tag, attrs, label) => {
      const element = document.createElementNS('http://www.w3.org/2000/svg', tag);
      for (const [key, value] of Object.entries(attrs)) element.setAttribute(key, value);
      if (label !== undefined) element.textContent = label;
      return element;
    };
    const interactive = (element, key, callback) => {
      rememberFocus(element, key);
      element.addEventListener('click', callback);
      element.addEventListener('keydown', event => {
        if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); callback(); }
      });
      return element;
    };
    const adjacent = adjacentBoundaries();
    const ordered = list => list.sort((a, b) => Number(b.key === state.boundaryKey) - Number(a.key === state.boundaryKey) || b.apiIds.length - a.apiIds.length || a.key.localeCompare(b.key));
    const allIncoming = ordered(adjacent.filter(boundary => boundary.to === state.componentId));
    const allOutgoing = ordered(adjacent.filter(boundary => boundary.from === state.componentId));
    const incoming = state.showAllNeighbors ? allIncoming : allIncoming.slice(0, 4);
    const outgoing = state.showAllNeighbors ? allOutgoing : allOutgoing.slice(0, 4);
    const height = Math.max(440, Math.max(incoming.length, outgoing.length) * 105 + 50);
    svg.setAttribute('viewBox', `0 0 970 ${height}`);
    svg.style.height = `${height}px`;
    const defs = shape('defs', {});
    for (const [suffix, fill] of [['', '#405a6c'], ['Selected', '#67dfbc']]) {
      const marker = shape('marker', {id: `arrow${suffix}`, markerWidth: 7, markerHeight: 7, refX: 6, refY: 3.5, orient: 'auto'});
      marker.append(shape('path', {d: 'M0 0 L7 3.5 L0 7', fill})); defs.append(marker);
    }
    svg.append(defs);
    const center = {x: 365, y: height / 2 - 31};
    const neighbors = [];
    for (const [list, side] of [[incoming, 'in'], [outgoing, 'out']]) {
      list.forEach((boundary, index) => {
        const pos = {x: side === 'in' ? 25 : 710, y: list.length === 1 ? center.y : 25 + index * (height - 112) / (list.length - 1)};
        const name = side === 'in' ? boundary.from : boundary.to;
        neighbors.push({pos, name, side});
        const x1 = side === 'in' ? pos.x + 235 : center.x + 245;
        const y1 = side === 'in' ? pos.y + 31 : center.y + 31;
        const x2 = side === 'in' ? center.x : pos.x;
        const y2 = side === 'in' ? center.y + 31 : pos.y + 31;
        const path = `M${x1} ${y1} C${(x1 + x2) / 2} ${y1} ${(x1 + x2) / 2} ${y2} ${x2} ${y2}`;
        const chosen = boundary.key === state.boundaryKey;
        const count = visibleApiIds(boundary).length;
        const label = `${boundary.from} → ${boundary.to}: ${count} APIs, ${boundary.calls.length} call sites`;
        const group = interactive(shape('g', {class: 'graph-boundary', tabindex: 0, role: 'button', 'aria-label': label, 'aria-pressed': String(chosen)}), `boundary:${boundary.key}`, () => chooseBoundary(boundary.key));
        group.append(shape('title', {}, label), shape('path', {class: `edge-line${chosen ? ' chosen' : ''}`, d: path, 'marker-end': `url(#arrow${chosen ? 'Selected' : ''})`}), shape('path', {class: 'edge-hit', d: path}), shape('rect', {class: `edge-pill${chosen ? ' chosen-pill' : ''}`, x: (x1 + x2) / 2 - 27, y: (y1 + y2) / 2 - 12, width: 54, height: 24, rx: 12}), shape('text', {class: `edge-count${chosen ? ' chosen' : ''}`, x: (x1 + x2) / 2, y: (y1 + y2) / 2 + 4}, `${count} API`));
        svg.append(group);
      });
    }
    const box = (pos, name, focus, suffix) => {
      const group = interactive(shape('g', {class: 'graph-node', tabindex: 0, role: 'button', 'aria-label': `Select component ${name}`, 'aria-pressed': String(focus)}), `graph-component:${suffix}:${name}`, () => chooseComponent(name));
      const label = name.length > 27 ? `${name.slice(0, 25)}…` : name;
      group.append(shape('title', {}, name), shape('rect', {class: `node-box${focus ? ' node-focus' : ''}`, x: pos.x, y: pos.y, width: focus ? 245 : 235, height: 62, rx: 8}), shape('text', {class: 'node-caption', x: pos.x + 14, y: pos.y + 19}, focus ? 'SELECTED COMPONENT' : 'COMPONENT'), shape('text', {class: 'node-label', x: pos.x + 14, y: pos.y + 42}, label));
      svg.append(group);
    };
    if (state.componentId) box(center, state.componentId, true, 'focus');
    for (const {pos, name, side} of neighbors) box(pos, name, false, side);
    if (!adjacent.length) svg.append(shape('text', {class: 'node-caption', x: 310, y: 70}, state.componentId ? 'No matching production boundaries. Browse owned APIs below.' : 'Select a component to view production boundaries.'));
    const hidden = adjacent.length - incoming.length - outgoing.length;
    byId('graph-note').textContent = `Arrows = production call direction · ${incoming.length + outgoing.length} of ${adjacent.length} matching directions shown${hidden ? ` · ${hidden} hidden, use Show all neighbors` : ''} · API counts are definitions; call-site counts are in arrow labels.`;
    byId('neighbors').textContent = state.showAllNeighbors ? 'Show up to 4 per direction' : `Show all neighbors (${adjacent.length})`;
    byId('neighbors').setAttribute('aria-pressed', String(state.showAllNeighbors));
    byId('component-title').textContent = state.componentId || 'All components';
  }
  function renderContracts() {
    const boundary = boundaries.get(state.boundaryKey);
    const contracts = byId('contracts'); contracts.replaceChildren();
    const ids = boundary ? visibleApiIds(boundary) : (state.componentId ? apisByComponent.get(state.componentId) || [] : data.apis).filter(api => matchesApi(api.id)).map(api => api.id);
    byId('boundary-title').textContent = boundary ? `${boundary.from} → ${boundary.to}` : `${state.componentId || 'All components'} · owned APIs`;
    byId('boundary-note').textContent = boundary ? `${ids.length} matching API definitions / ${boundary.apiIds.length} total · ${boundary.calls.length} production call sites. Select an API.` : `${ids.length} matching API definitions. Select a production arrow above to inspect a boundary.`;
    const pageKey = `contracts:${state.boundaryKey || state.componentId || 'all'}`;
    if (ids.indexOf(state.apiId) >= (pageLimits.get(pageKey) || 40)) pageLimits.set(pageKey, ids.indexOf(state.apiId) + 1);
    renderPaged(contracts, ids, pageKey, id => {
      const api = apiById.get(id); const tests = testsByTarget.get(id) || [];
      const selected = state.apiId === id;
      const button = action(undefined, () => chooseApi(id), `chip${selected ? ' selected' : ''}`, `api-chip:${id}`);
      button.setAttribute('aria-label', `${api.name} · ${api.kind} · ${tests.length} declared target tests`);
      button.setAttribute('aria-pressed', String(selected));
      button.title = `${apiLabel(id)}\n${api.kind}\n${id}`;
      button.append(node('span', undefined, `dot${tests.length ? '' : ' amber'}`), node('span', shortName(api.name)), node('span', String(tests.length), 'count'));
      return button;
    });
    if (!ids.length) empty(contracts, 'No matching APIs. All tests, including untargeted tests, remain available in Browse all APIs and tests.');
  }
  function callRow(call, otherId) {
    const row = node('div', undefined, 'call-row');
    if (apiById.has(otherId)) row.append(action(apiLabel(otherId), () => chooseApi(otherId, true)));
    else row.append(node('span', apiLabel(otherId)));
    row.append(node('span', call.dispatch, 'tag'), sourceButton(call), node('div', originLabel(call), 'meta'));
    return row;
  }
  function renderInspector() {
    const detail = byId('selection-detail'); detail.replaceChildren();
    const api = apiById.get(state.apiId);
    byId('selection-title').textContent = api ? shortName(api.name) : 'Select an API';
    if (!api) { empty(detail, 'No API matches the current selection and filters. Browse captured tests and source below.'); return; }
    const boundary = boundaries.get(state.boundaryKey);
    const tests = testsByTarget.get(api.id) || [];
    detail.append(node('p', api.name, 'path'), node('p', api.kind, 'path'), node('span', tests.length ? `${tests.length} tests declare this exact API target` : 'No declared target tests', `badge${tests.length ? '' : ' missing'}`));
    const route = node('div', undefined, 'route');
    if (boundary) {
      route.append(node('span', `Caller component · ${boundary.from}`), node('strong', `↓ ${(boundary.callsByTarget.get(api.id) || []).length} production call sites to this API`), node('span', `Definition component · ${boundary.to}`));
    } else route.append(node('span', `Definition component · ${api.package}`), node('span', 'Owned API selection · no boundary selected'));
    detail.append(route, node('p', `${api.path}:${api.line}`, 'path'), sourceButton(api, 'Open API definition ↗', 'source-button'));
    const title = node('div', undefined, 'test-title'); title.append(node('span', 'Declared target tests'), node('span', String(tests.length))); detail.append(title);
    const links = node('div', undefined, 'test-links');
    renderPaged(links, tests, `evidence:${api.id}`, test => {
      const button = sourceButton(test, undefined, 'test-card'); button.replaceChildren();
      button.setAttribute('aria-label', `Open test source ${test.name}`);
      button.append(node('span', test.name, 'test-name'), node('small', `${test.package} · ${test.path}:${test.line}`), node('span', 'Declared target · execution not captured', 'badge'));
      return button;
    }, 12);
    detail.append(links);
    if (!tests.length) empty(detail, 'No test in this snapshot declares this exact API ID as a target.');
    detail.append(node('p', 'Calls are compiler observations. Declared targets record author intent. This snapshot contains no test execution results and does not establish sufficient verification.', 'quiet'));
    const identity = node('details');
    identity.append(node('summary', 'Definition identity'), node('p', api.id, 'path'));
    detail.append(identity);
    if (boundary) addCallDetails(detail, `Selected boundary call sites (${(boundary.callsByTarget.get(api.id) || []).length})`, boundary.callsByTarget.get(api.id) || [], 'caller', `boundary-calls:${api.id}`);
    const groups = callsByApi.get(api.id);
    for (const [titleText, key, field] of [['Production callers', 'productionCallers', 'caller'], ['Production calls', 'productionCalls', 'callee'], ['Other compilation contexts: callers', 'otherCallers', 'caller'], ['Other compilation contexts: calls', 'otherCalls', 'callee']]) addCallDetails(detail, `${titleText} (${groups[key].length})`, groups[key], field, `${key}:${api.id}`);
    const references = referencesByApi.get(api.id) || [];
    addCallDetails(detail, `Function references (${references.length})`, references, 'caller', `references:${api.id}`);
  }
  function addCallDetails(parent, title, records, field, key) {
    const disclosure = node('details');
    disclosure.append(node('summary', title));
    const body = node('div');
    renderPaged(body, records, key, call => callRow(call, call[field]), 20);
    if (!records.length) empty(body, 'No workspace records in this scope.');
    disclosure.append(body); parent.append(disclosure);
  }
  function renderCatalog() {
    const apis = (state.componentId ? apisByComponent.get(state.componentId) || [] : data.apis).filter(api => matchesApi(api.id));
    const tests = (state.componentId ? testsByComponent.get(state.componentId) || [] : data.tests).filter(test => (state.testKind === 'all' || Boolean(test.targets.length) === (state.testKind === 'targeted')) && (!state.query || testSearch.get(test).includes(state.query)));
    byId('api-count').textContent = `(${apis.length})`; byId('test-count').textContent = `(${tests.length})`;
    byId('catalog-count').textContent = `· ${apis.length} APIs / ${tests.length} tests in current scope`;
    const apiList = byId('apis'); const testList = byId('tests'); apiList.replaceChildren(); testList.replaceChildren();
    renderPaged(apiList, apis, 'catalog-apis', api => {
      const row = node('div', undefined, 'item');
      row.append(action(api.name, () => chooseApi(api.id, true)), node('div', `${api.package} · ${api.kind}`, 'meta'), sourceButton(api)); return row;
    });
    renderPaged(testList, tests, 'catalog-tests', test => {
      const row = node('div', undefined, 'item');
      row.append(node('strong', test.name), node('div', `${test.package} · ${test.target_kind} ${test.target_name}`, 'meta'), node('span', test.targets.length ? 'Targeted' : 'Untargeted / 対象未指定', `tag${test.targets.length ? ' targeted' : ''}`), sourceButton(test));
      for (const id of test.targets) {
        if (apiById.has(id)) row.append(action(apiLabel(id), () => chooseApi(id, true)));
        else row.append(node('span', id, 'path'));
      }
      return row;
    });
    if (!apis.length) empty(apiList, 'No matching APIs.');
    if (!tests.length) empty(testList, 'No matching tests.');
  }
  function renderSourceSearch() {
    const results = byId('source-results'); results.replaceChildren();
    if (!state.query) { byId('source-count').textContent = ''; empty(results, 'Enter text above to search the captured Rust source.'); return; }
    if (sourceSearchCache.query !== state.query || sourceSearchCache.componentId !== state.componentId) {
      const paths = state.componentId ? filesByComponent.get(state.componentId) || new Set() : fileLines.keys();
      const matches = [];
      for (const path of paths) for (const [index, line] of (fileLines.get(path) || []).entries()) if (line.toLowerCase().includes(state.query)) matches.push({path, line: index + 1, text: line});
      sourceSearchCache = {query: state.query, componentId: state.componentId, matches};
    }
    const matches = sourceSearchCache.matches;
    byId('source-count').textContent = `(${matches.length})`;
    renderPaged(results, matches, 'source-results', result => {
      const row = node('div', undefined, 'call-row'); row.append(sourceButton(result), node('code', ` ${result.text.trim()}`)); return row;
    }, 80);
    if (!matches.length) empty(results, 'No matching source lines.');
  }
  function render() {
    const focusKey = document.activeElement?.dataset.focusKey;
    const disclosures = [...byId('selection-detail').querySelectorAll('details')].filter(element => element.open).map(element => element.firstElementChild.textContent);
    focusTargets = new Map();
    reconcile();
    byId('gaps').setAttribute('aria-pressed', String(state.gapsOnly));
    renderComponents(); renderGraph(); renderContracts(); renderInspector(); renderCatalog(); renderSourceSearch();
    for (const element of byId('selection-detail').querySelectorAll('details')) if (disclosures.includes(element.firstElementChild.textContent)) element.open = true;
    if (focusKey) {
      const target = focusTargets.get(focusKey) || focusTargets.get(`api-chip:${state.apiId}`) || focusTargets.get(`component:${state.componentId}`);
      if (target) target.focus({preventScroll: true});
    }
  }
  byId('summary').textContent = `${data.components.length} components · ${boundaries.size} production directions · ${productionCallCount} production calls / ${data.calls.length} total · ${data.tests.length} tests · execution not captured`;
  byId('component-count').textContent = String(data.components.length);
  byId('scope').textContent = `${data.configuration.scope} · Dynamic calls retain the declared trait interface. Same-package calls and other compilation contexts remain in API details.`;
  for (const component of data.components) { const option = node('option', component.id); option.value = component.id; byId('component').append(option); }
  byId('unresolved-title').textContent = `Unresolved calls (${data.unresolved.length})`;
  for (const item of data.unresolved) {
    const row = node('div', undefined, 'call-row');
    row.append(node('div', `${apiLabel(item.caller)} · ${item.reason}`), sourceButton(item), node('div', originLabel(item), 'meta')); byId('unresolved').append(row);
  }
  if (!data.unresolved.length) empty(byId('unresolved'), 'No unresolved workspace calls recorded.');
  byId('configuration').append(node('p', data.configuration.toolchain), node('p', `Compiler accounting: ${JSON.stringify(data.coverage)}`));
  for (const command of data.configuration.cargo) byId('configuration').append(node('div', `cargo ${command}`, 'meta'));
  for (const unit of data.units) byId('configuration').append(node('div', `${unit.package} · ${unit.target_kind} ${unit.target_name} · ${unit.is_test ? 'test harness' : 'normal'} · features: ${unit.features.join(', ') || 'none'} · purposes: ${unit.purposes.join(', ') || 'build script'} · ${unit.source}`, 'meta'));
  for (const target of data.excluded_targets) byId('configuration').append(node('div', `Excluded: ${target.package} · ${target.target_kind} ${target.target_name} · ${target.reason} · required features: ${target.required_features.join(', ') || 'none'}`, 'meta'));
  byId('search').addEventListener('input', event => { state.query = event.target.value.trim().toLowerCase(); pageLimits.clear(); render(); });
  byId('gaps').addEventListener('click', () => { state.gapsOnly = !state.gapsOnly; pageLimits.clear(); render(); });
  byId('neighbors').addEventListener('click', () => { state.showAllNeighbors = !state.showAllNeighbors; render(); });
  byId('component').addEventListener('change', event => { pageLimits.clear(); chooseComponent(event.target.value); });
  byId('test-kind').addEventListener('change', event => { state.testKind = event.target.value; pageLimits.delete('catalog-tests'); renderCatalog(); });
  byId('close-source').addEventListener('click', () => byId('source-panel').close());
  const initial = [...boundaries.values()].find(boundary => boundary.apiIds.some(id => (testsByTarget.get(id) || []).length)) || boundaries.values().next().value;
  state.componentId = initial?.from || data.components[0]?.id || null;
  state.boundaryKey = initial?.key || null;
  render();
})();
