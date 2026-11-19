'use strict';
(() => {
  const snapshot = JSON.parse(document.getElementById('traceability-data').textContent);
  const data = snapshot.bundle;
  const apiById = new Map(data.apis.map(api => [api.id, api]));
  const productionCall = call => call.origins.some(unit => unit.purposes.includes('production') && !unit.is_test && ['lib', 'bin'].includes(unit.target_kind));
  const productionCalls = data.calls.filter(productionCall);
  const originLabel = fact => fact.origins.map(unit => `${unit.package} ${unit.target_kind} ${unit.target_name} (${unit.is_test ? 'test harness' : 'normal'}, features: ${unit.features.join(', ') || 'none'}, purposes: ${unit.purposes.join(', ') || 'build script'})`).join(', ');
  const byId = id => document.getElementById(id);
  const node = (tag, value, className) => {
    const element = document.createElement(tag);
    if (value !== undefined) element.textContent = value;
    if (className) element.className = className;
    return element;
  };
  const empty = (element, message) => element.append(node('p', message, 'empty'));
  const sourceButton = location => {
    const button = node('button', `${location.path}:${location.line}`, 'source-link');
    button.type = 'button';
    button.addEventListener('click', () => showSource(location));
    return button;
  };
  const action = (label, callback) => {
    const button = node('button', label);
    button.type = 'button';
    button.addEventListener('click', callback);
    return button;
  };
  function showSource(location) {
    const file = snapshot.files[location.path];
    if (!file) return;
    byId('source-panel').hidden = false;
    byId('source-title').textContent = `${location.path}:${location.line}`;
    byId('source-status').textContent = file.status;
    byId('github').hidden = !file.github_url;
    if (file.github_url) byId('github').href = `${file.github_url}#L${location.line}`;
    const code = byId('source-code');
    code.replaceChildren();
    const lines = file.content.split('\n');
    let selected;
    lines.forEach((value, index) => {
      const line = node('span', undefined, `source-line${index + 1 === location.line ? ' highlight' : ''}`);
      line.append(node('span', String(index + 1), 'line-number'), document.createTextNode(value || ' '));
      code.append(line);
      if (index + 1 === location.line) selected = line;
    });
    byId('source-panel').scrollIntoView({block: 'nearest'});
    if (selected) code.scrollTop = Math.max(0, selected.offsetTop - code.offsetTop - code.clientHeight / 3);
  }
  function apiLabel(identity) {
    const api = apiById.get(identity);
    return api ? `${api.package} · ${api.name}` : identity;
  }
  function apiGraph(api, tests) {
    const namespace = 'http://www.w3.org/2000/svg';
    const shape = (tag, attributes, label) => {
      const element = document.createElementNS(namespace, tag);
      for (const [key, value] of Object.entries(attributes)) element.setAttribute(key, value);
      if (label !== undefined) element.textContent = label;
      return element;
    };
    const height = Math.max(180, tests.length * 60 + 40);
    const svg = shape('svg', {viewBox: `0 0 1100 ${height}`, role: 'img', 'aria-label': 'Component ownership and explicit test targets'});
    svg.style.height = `${Math.min(height, 500)}px`;
    const box = (x, y, width, label, callback) => {
      const group = shape('g', {class: 'graph-node', tabindex: 0, role: 'button', 'aria-label': label});
      group.append(shape('rect', {x, y, width, height: 38, rx: 6}), shape('text', {x: x + 10, y: y + 24}, label.length > 65 ? `${label.slice(0, 62)}…` : label), shape('title', {}, label));
      group.addEventListener('click', callback);
      group.addEventListener('keydown', event => { if (event.key === 'Enter') callback(); });
      svg.append(group);
    };
    const apiY = height / 2;
    svg.append(shape('path', {d: `M840,48 L840,${apiY}`, class: 'graph-edge', 'marker-end': 'url(#arrow)'}));
    svg.append(shape('text', {x: 850, y: (48 + apiY) / 2}, 'owns'));
    tests.forEach((test, index) => {
      const y = 20 + index * 60;
      svg.append(shape('path', {d: `M530,${y + 19} L700,${apiY + 19}`, class: 'graph-edge', 'marker-end': 'url(#arrow)'}));
      svg.append(shape('text', {x: 540, y: y + 13}, 'targets'));
      box(10, y, 520, `${test.package} · ${test.name}`, () => showSource(test));
    });
    box(700, 10, 380, `Component · ${api.package}`, () => { byId('component').value = api.package; render(); });
    box(700, apiY, 380, api.name, () => showSource(api));
    const wrapper = node('div', undefined, 'graph-wrap'); wrapper.append(svg);
    return wrapper;
  }
  function showApi(api) {
    byId('selection').hidden = false;
    byId('selection-title').textContent = apiLabel(api.id);
    const detail = byId('selection-detail');
    detail.replaceChildren(node('p', api.kind), sourceButton(api));
    const tests = data.tests.filter(test => test.targets.includes(api.id));
    detail.append(apiGraph(api, tests));
    detail.append(node('h3', `Explicit test targets (${tests.length})`));
    for (const test of tests) {
      const row = node('div', undefined, 'call-row');
      row.append(node('div', `${test.package} · ${test.name}`), sourceButton(test));
      detail.append(row);
    }
    if (!tests.length) empty(detail, 'No test declares this API as a target.');
    for (const [scope, records] of [['Production', productionCalls], ['Other compilation contexts', data.calls.filter(call => !productionCall(call))]]) {
      for (const [label, field, direction] of [['callers', 'caller', 'callee'], ['calls', 'callee', 'caller']]) {
        const calls = records.filter(call => call[direction] === api.id);
        detail.append(node('h3', `${scope} ${label} (${calls.length})`));
        for (const call of calls) {
          const row = node('div', undefined, 'call-row');
          row.append(action(apiLabel(call[field]), () => showApi(apiById.get(call[field]))),
            node('span', call.dispatch, 'tag'), sourceButton(call), node('div', originLabel(call), 'meta'));
          detail.append(row);
        }
        if (!calls.length) empty(detail, 'No workspace calls recorded in this scope.');
      }
    }
    const references = (data.references || []).filter(reference => reference.callee === api.id);
    detail.append(node('h3', `Function references (${references.length})`));
    for (const reference of references) {
      const row = node('div', undefined, 'call-row');
      row.append(action(apiLabel(reference.caller), () => showApi(apiById.get(reference.caller))), sourceButton(reference), node('div', originLabel(reference), 'meta'));
      detail.append(row);
    }
  }
  function renderGraph() {
    const svg = byId('graph');
    svg.replaceChildren();
    const namespace = 'http://www.w3.org/2000/svg';
    const shape = (tag, attrs) => {
      const element = document.createElementNS(namespace, tag);
      for (const [key, value] of Object.entries(attrs)) element.setAttribute(key, value);
      return element;
    };
    const width = 1100;
    const columns = 4;
    const height = Math.max(120, Math.ceil(data.components.length / columns) * 105 + 40);
    svg.setAttribute('viewBox', `0 0 ${width} ${height}`);
    svg.style.height = `${Math.min(height, 620)}px`;
    const defs = shape('defs', {});
    const marker = shape('marker', {id: 'arrow', viewBox: '0 0 10 10', refX: 9, refY: 5, markerWidth: 6, markerHeight: 6, orient: 'auto-start-reverse'});
    marker.append(shape('path', {d: 'M 0 0 L 10 5 L 0 10 z', fill: '#9aacbc'}));
    defs.append(marker); svg.append(defs);
    const positions = new Map(data.components.map((component, index) => [component.id, {x: 30 + (index % columns) * 270, y: 25 + Math.floor(index / columns) * 105}]));
    const edges = new Map();
    for (const call of productionCalls) {
      const caller = apiById.get(call.caller).package;
      const callee = apiById.get(call.callee).package;
      if (caller === callee) continue;
      const key = JSON.stringify([caller, callee]);
      edges.set(key, (edges.get(key) || 0) + 1);
    }
    const selected = byId('component').value;
    for (const [key, count] of edges) {
      const [caller, callee] = JSON.parse(key);
      const from = positions.get(caller); const to = positions.get(callee);
      if (!from || !to) continue;
      const path = shape('path', {d: `M${from.x + 116},${from.y + 42} Q${(from.x + to.x) / 2 + 116},${Math.min(from.y, to.y) - 16} ${to.x + 116},${to.y}`, class: 'graph-edge', 'marker-end': 'url(#arrow)'});
      if (selected && selected !== caller && selected !== callee) path.style.opacity = '.12';
      const title = shape('title', {}); title.textContent = `${caller} → ${callee}: ${count} call sites`; path.append(title); svg.append(path);
    }
    for (const component of data.components) {
      const position = positions.get(component.id);
      const group = shape('g', {class: `graph-node${selected === component.id ? ' selected' : ''}`, role: 'button', tabindex: 0, 'aria-label': `Inspect ${component.id}`});
      group.append(shape('rect', {x: position.x, y: position.y, width: 232, height: 42, rx: 7}));
      const label = shape('text', {x: position.x + 12, y: position.y + 26}); label.textContent = component.id; group.append(label);
      const choose = () => { byId('component').value = selected === component.id ? '' : component.id; render(); };
      group.addEventListener('click', choose);
      group.addEventListener('keydown', event => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); choose(); } });
      svg.append(group);
    }
    byId('graph-note').textContent = `${edges.size} production component call directions. Only calls observed during the production build in normal library or binary units create arrows. All compilation contexts remain visible in API details. Dynamic calls point to the declared trait interface.`;
  }
  function render() {
    const query = byId('search').value.trim().toLowerCase();
    const component = byId('component').value;
    const kind = byId('test-kind').value;
    const apis = data.apis.filter(api => (!component || api.package === component) && (!query || `${api.name} ${api.path} ${api.package}`.toLowerCase().includes(query)));
    const tests = data.tests.filter(test => (!component || test.package === component || test.targets.some(id => apiById.get(id).package === component)) && (kind === 'all' || Boolean(test.targets.length) === (kind === 'targeted')) && (!query || `${test.name} ${test.path} ${test.package} ${test.targets.map(apiLabel).join(' ')}`.toLowerCase().includes(query)));
    byId('api-count').textContent = `(${apis.length})`;
    byId('test-count').textContent = `(${tests.length})`;
    byId('apis').replaceChildren(); byId('tests').replaceChildren();
    for (const api of apis) {
      const row = node('div', undefined, 'item');
      row.append(action(api.name, () => showApi(api)), node('div', `${api.package} · ${api.kind}`, 'meta'), sourceButton(api));
      byId('apis').append(row);
    }
    for (const test of tests) {
      const row = node('div', undefined, 'item');
      row.append(node('strong', test.name), node('div', `${test.package} · ${test.target_kind} ${test.target_name}`, 'meta'), node('span', test.targets.length ? 'Targeted' : 'Untargeted / 対象未指定', `tag${test.targets.length ? ' targeted' : ''}`), sourceButton(test));
      for (const id of test.targets) row.append(action(apiLabel(id), () => showApi(apiById.get(id))));
      byId('tests').append(row);
    }
    if (!apis.length) empty(byId('apis'), 'No matching APIs.');
    if (!tests.length) empty(byId('tests'), 'No matching tests.');
    const results = byId('source-results'); results.replaceChildren();
    let count = 0;
    if (query) {
      for (const [path, file] of Object.entries(snapshot.files)) {
        if (component && !data.units.some(unit => unit.package === component && unit.source === path) && !data.apis.some(api => api.package === component && api.path === path) && !data.tests.some(test => test.package === component && test.path === path)) continue;
        file.content.split('\n').forEach((line, index) => {
          if (count >= 80 || !line.toLowerCase().includes(query)) return;
          const row = node('div', undefined, 'call-row');
          row.append(sourceButton({path, line: index + 1}), node('code', ` ${line.trim()}`));
          results.append(row); count += 1;
        });
      }
      if (!count) empty(results, 'No matching source lines.');
      if (count === 80) results.append(node('p', 'Showing the first 80 source matches. Narrow the search to inspect more.'));
    } else empty(results, 'Enter text to search the captured Rust source.');
    renderGraph();
  }
  byId('summary').textContent = `${data.components.length} components · ${data.apis.length} APIs · ${productionCalls.length} production calls · ${data.calls.length} total workspace calls · ${data.tests.length} tests (${data.tests.filter(test => test.targets.length).length} targeted)`;
  byId('scope').textContent = data.configuration.scope;
  for (const component of data.components) {
    const option = node('option', component.id); option.value = component.id; byId('component').append(option);
  }
  byId('unresolved-title').textContent = `Unresolved calls (${data.unresolved.length})`;
  for (const item of data.unresolved) {
    const row = node('div', undefined, 'call-row'); row.append(node('div', `${apiLabel(item.caller)} · ${item.reason}`), sourceButton(item), node('div', originLabel(item), 'meta')); byId('unresolved').append(row);
  }
  if (!data.unresolved.length) empty(byId('unresolved'), 'No unresolved workspace calls recorded.');
  byId('configuration').append(node('p', data.configuration.toolchain), node('p', `Compiler accounting: ${JSON.stringify(data.coverage)}`));
  for (const command of data.configuration.cargo) byId('configuration').append(node('div', `cargo ${command}`, 'meta'));
  for (const unit of data.units) byId('configuration').append(node('div', `${unit.package} · ${unit.target_kind} ${unit.target_name} · ${unit.is_test ? 'test harness' : 'normal'} · features: ${unit.features.join(', ') || 'none'} · purposes: ${unit.purposes.join(', ') || 'build script'} · ${unit.source}`, 'meta'));
  for (const target of data.excluded_targets) byId('configuration').append(node('div', `Excluded: ${target.package} · ${target.target_kind} ${target.target_name} · ${target.reason} · required features: ${target.required_features.join(', ') || 'none'}`, 'meta'));
  byId('search').addEventListener('input', render);
  byId('component').addEventListener('change', render);
  byId('test-kind').addEventListener('change', render);
  byId('close-source').addEventListener('click', () => { byId('source-panel').hidden = true; });
  render();
})();
