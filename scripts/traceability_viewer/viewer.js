"use strict";
(() => {
  const data = JSON.parse(document.getElementById("viewer-data").textContent);
  const bundle = data.bundle;
  const byId = id => document.getElementById(id);
  const features = new Map(bundle.features.map(node => [node.id, node]));
  const components = new Map(bundle.components.map(node => [node.id, node]));
  const contracts = new Map(bundle.contracts.map(node => [node.id, node]));
  const tests = new Map(bundle.tests.map(node => [node.id, node]));
  const kinds = { feature: "機能", component: "コンポーネント", guarantee: "保証", contract: "契約", test: "テスト" };
  const classifications = { boundary: "境界テスト", internal: "内部テスト", infrastructure: "基盤検査", helper: "補助テスト" };
  const relationships = { asserts: "事例を直接検証", exercises: "処理を通る", unreviewed: "未レビュー", requires: "下位契約を利用", root: "保証の入口", membership: "所属" };
  const evidence = new Map(bundle.contracts.map(node => [node.id, []]));
  for (const test of tests.values()) {
    if (test.classification !== "boundary") continue;
    for (const item of test.semantics.evidence) evidence.get(item.contract).push({ ...item, test: test.id });
  }
  let selection;
  let graph;
  let zoom = 1;
  let dimensions = { width: 1100, height: 500 };
  let sourceOpener;

  function element(tag, text, className) {
    const result = document.createElement(tag);
    if (text !== undefined) result.textContent = text;
    if (className) result.className = className;
    return result;
  }
  function button(text, action, className) {
    const result = element("button", text, className);
    result.type = "button";
    result.addEventListener("click", action);
    return result;
  }
  function title(node) { return node.title || (node.name ? node.name.split("::").at(-1) : node.id); }
  function key(kind, id) { return `${kind}:${id}`; }
  function entry() { return selection.feature ? features.get(selection.feature) : components.get(selection.component); }
  function request(kind, id, extra = {}) { return { feature: selection.feature, component: selection.component, kind, id, ...extra }; }
  function buildGraph(state) {
    const nodes = new Map();
    const edges = new Map();
    const depths = new Map();
    const roots = [];
    function add(kind, node, depth) {
      const id = key(kind, node.id);
      nodes.set(id, { kind, node });
      depths.set(id, Math.max(depths.get(id) || 0, depth));
      return id;
    }
    function edge(from, to, relationship, detail = {}) {
      const id = JSON.stringify([from, to, relationship]);
      edges.set(id, { id, from, to, relationship, ...detail });
    }
    function visit(id, depth) {
      const node = contracts.get(id);
      const existing = depths.get(key("contract", id));
      if (existing !== undefined && existing >= depth) return key("contract", id);
      const from = add("contract", node, depth);
      for (const dependency of node.requires) {
        const to = visit(dependency.contract, depth + 1);
        edge(from, to, "requires", { reason: dependency.reason });
      }
      return from;
    }
    if (state.feature) {
      for (const guarantee of features.get(state.feature).guarantees) {
        const from = add("guarantee", guarantee, 0);
        for (const id of guarantee.roots) {
          roots.push(id);
          edge(from, visit(id, 1), "root");
        }
      }
    } else {
      const component = components.get(state.component);
      for (const node of contracts.values()) {
        if (node.owner === component.id || node.consumer === component.id || node.provider === component.id) {
          roots.push(node.id);
          visit(node.id, 0);
        }
      }
    }
    const context = state.context || (state.kind === "contract" ? state.id : null);
    const testDepth = Math.max(0, ...depths.values()) + 1;
    if (context && nodes.has(key("contract", context))) {
      for (const item of evidence.get(context)) {
        const to = add("test", tests.get(item.test), testDepth);
        edge(key("contract", context), to, item.relationship, { evidence: item });
      }
    }
    if (state.component) {
      for (const test of tests.values()) {
        if (test.package === state.component && test.classification !== "boundary") add("test", test, testDepth);
      }
    }
    return { nodes, edges, depths, roots };
  }
  function normalized(state) {
    if (state.feature ? !features.has(state.feature) : !components.has(state.component)) return null;
    const base = state.feature ? { feature: state.feature } : { component: state.component };
    const kind = state.kind || (state.feature ? "feature" : "component");
    const id = state.id || state.feature || state.component;
    const candidate = { ...base, kind, id };
    if (state.context && ["test", "edge"].includes(kind)) candidate.context = state.context;
    const visible = buildGraph(candidate);
    if (kind === "edge") {
      const edge = visible.edges.get(state.edge);
      if (!edge) return null;
      return { ...base, kind, edge: edge.id, ...(state.context ? { context: state.context } : {}) };
    }
    if ((kind === "feature" && id === state.feature) || (kind === "component" && id === state.component)) return candidate;
    if (!visible.nodes.has(key(kind, id))) return null;
    if (candidate.context && !visible.nodes.has(key("contract", candidate.context))) return null;
    if (kind === "test" && tests.get(id).classification === "boundary" && !evidence.get(candidate.context)?.some(item => item.test === id)) return null;
    return candidate;
  }
  function hashFor(state) { return `#${new URLSearchParams(Object.entries(state).filter(([, value]) => value !== undefined && value !== null))}`; }
  function select(state, mode = "push") {
    let next = normalized(state);
    const invalid = !next;
    if (!next) {
      const feature = features.get(state.feature) || features.values().next().value;
      const component = components.get(state.component) || components.values().next().value;
      next = feature ? { feature: feature.id, kind: "feature", id: feature.id } : { component: component.id, kind: "component", id: component.id };
    }
    byId("notice").hidden = !invalid;
    byId("notice").textContent = invalid ? "URL の選択項目が見つからないため、入口の詳細を表示しています。" : "";
    if (selection?.feature !== next.feature || selection?.component !== next.component) zoom = 1;
    selection = next;
    graph = buildGraph(selection);
    if (mode === "push" && location.hash !== hashFor(next)) history.pushState(null, "", hashFor(next));
    if (mode === "replace") history.replaceState(null, "", hashFor(next));
    renderEntries();
    renderGraph();
    renderDetails();
  }
  function restore() {
    const state = Object.fromEntries(new URLSearchParams(location.hash.slice(1)));
    if (!location.hash) {
      const first = features.keys().next().value;
      const component = components.keys().next().value;
      select(first ? { feature: first, kind: "feature", id: first } : { component, kind: "component", id: component }, "replace");
    } else select(state, "none");
  }
  function renderEntries() {
    const query = byId("feature-search").value.trim().toLocaleLowerCase();
    const container = byId("feature-list");
    container.replaceChildren();
    let count = 0;
    for (const [kind, group] of [["feature", features], ["component", components]]) {
      container.append(element("h3", kinds[kind]));
      for (const [id, node] of group) {
        if (!`${id} ${title(node)} ${node.description || ""}`.toLocaleLowerCase().includes(query)) continue;
        count++;
        const control = button(title(node), () => select({ [kind]: id, kind, id }), "feature-button");
        control.dataset.entryKind = kind;
        control.dataset.entryId = id;
        control.setAttribute("aria-current", String(selection[kind] === id));
        control.append(element("small", id));
        container.append(control);
      }
    }
    byId("feature-count").textContent = `${count} / ${features.size + components.size} 件`;
    if (!count) container.append(element("p", "一致する項目がありません。", "muted"));
  }
  function renderGraph() {
    byId("feature-id").textContent = `${kinds[selection.feature ? "feature" : "component"]} · ${entry().id}`;
    byId("feature-title").textContent = title(entry());
    byId("feature-details").textContent = "入口の説明";
    const canvas = byId("graph");
    const focus = canvas.contains(document.activeElement);
    canvas.replaceChildren();
    const columns = new Map();
    for (const [id, depth] of graph.depths) {
      if (!columns.has(depth)) columns.set(depth, []);
      columns.get(depth).push(id);
    }
    const positions = new Map();
    for (const [depth, ids] of columns) ids.forEach((id, row) => positions.set(id, { x: 20 + depth * 340, y: 54 + row * 148 }));
    dimensions = { width: Math.max(600, ...[...positions.values()].map(pos => pos.x + 260)), height: Math.max(400, ...[...positions.values()].map(pos => pos.y + 148)) };
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("width", dimensions.width);
    svg.setAttribute("height", dimensions.height);
    svg.setAttribute("aria-hidden", "true");
    canvas.append(svg);
    for (const edge of graph.edges.values()) {
      const from = positions.get(edge.from);
      const to = positions.get(edge.to);
      const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
      const start = from.x + 240;
      path.setAttribute("d", `M ${start} ${from.y + 55} C ${start + 45} ${from.y + 55}, ${to.x - 45} ${to.y + 55}, ${to.x} ${to.y + 55}`);
      path.setAttribute("fill", "none");
      path.setAttribute("stroke", edge.evidence ? "#218f86" : "#aabed5");
      path.setAttribute("stroke-width", "1.8");
      if (edge.relationship === "exercises") path.setAttribute("stroke-dasharray", "7 4");
      if (edge.relationship === "unreviewed") path.setAttribute("stroke-dasharray", "2 4");
      svg.append(path);
      const control = button(edge.relationship === "requires" ? "利用" : edge.relationship === "root" ? "入口" : edge.relationship, () => select(request("edge", undefined, { edge: edge.id, context: edge.evidence ? edge.evidence.contract : selection.context })), "edge-button");
      control.dataset.relationship = edge.relationship;
      control.dataset.from = edge.from;
      control.dataset.to = edge.to;
      control.setAttribute("aria-label", `${edge.from} → ${edge.to} ${relationships[edge.relationship]}`);
      control.setAttribute("aria-pressed", String(selection.edge === edge.id));
      control.style.left = `${start + 10}px`;
      control.style.top = `${(from.y + to.y) / 2 + 12}px`;
      canvas.append(control);
    }
    for (const [id, position] of positions) {
      const { kind, node } = graph.nodes.get(id);
      const control = button(undefined, () => select(request(kind, node.id, { context: kind === "test" ? selection.context || (selection.kind === "contract" ? selection.id : undefined) : undefined })), "graph-node");
      control.dataset.kind = kind;
      control.dataset.nodeId = node.id;
      control.setAttribute("aria-label", `${kinds[kind]} ${title(node)}`);
      control.setAttribute("aria-pressed", String(selection.kind === kind && selection.id === node.id));
      control.style.left = `${position.x}px`;
      control.style.top = `${position.y}px`;
      control.append(element("span", kind === "test" ? classifications[node.classification] : kinds[kind], "node-kind"), element("span", title(node), "node-title"));
      control.append(element("span", kind === "contract" ? `${node.consumer} → ${node.provider}` : node.statement || node.package, "node-caption"));
      if (node.gap) control.append(element("span", "未検証事項あり", "gap-mark"));
      canvas.append(control);
    }
    if (!positions.size) canvas.append(element("p", "この入口には契約・テストの宣言がありません。", "empty-graph"));
    applyZoom();
    if (focus) canvas.querySelector('[aria-pressed="true"]')?.focus({ preventScroll: true });
    canvas.querySelector('.graph-node[aria-pressed="true"]')?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }
  function paragraph(container, heading, text, className) { container.append(element("h3", heading), element("p", text, className)); }
  function list(container, heading, values, ordered = false) {
    if (!values?.length) return;
    container.append(element("h3", heading));
    const result = element(ordered ? "ol" : "ul");
    for (const value of values) result.append(element("li", value));
    container.append(result);
  }
  function nodeLink(container, kind, node, context, label) {
    const control = button(label || `${kind === "test" ? classifications[node.classification] : kinds[kind]} · ${title(node)}`, () => select(kind === "contract" && !graph.nodes.has(key(kind, node.id)) ? { component: node.owner, kind, id: node.id } : request(kind, node.id, { context })));
    control.dataset.relatedId = node.id;
    container.append(control);
  }
  function sourceButton(container, reference, label = "ソースを開く") {
    if (!reference) return;
    const location = typeof reference === "string" ? { path: reference, line: 1 } : { path: reference.path, line: reference.line || data.locations[reference.path][reference.needle] };
    const control = button(label, () => openSource(location, control));
    control.dataset.sourceOpen = "true";
    control.dataset.sourcePath = location.path;
    control.dataset.sourceLine = location.line;
    container.append(control);
  }
  function showEvidence(container, item) {
    paragraph(container, "検証範囲", item.scope);
    paragraph(container, "根拠", item.reason);
    paragraph(container, "観測方法", item.observation === "ProviderBoundary" ? "ProviderBoundary · harness が呼出元を代行" : "CompleteInteraction · 実呼出元と実提供元の協調");
    list(container, "実コンポーネント", item.real);
    list(container, "fake コンポーネント", item.fake);
  }
  function renderDetails() {
    const details = byId("details");
    details.replaceChildren();
    if (selection.kind === "edge") {
      const edge = graph.edges.get(selection.edge);
      details.append(element("p", "関係", "eyebrow"), element("h2", relationships[edge.relationship]));
      for (const id of [edge.from, edge.to]) {
        const node = graph.nodes.get(id);
        nodeLink(details, node.kind, node.node, edge.evidence?.contract);
      }
      if (edge.reason) paragraph(details, "利用する理由", edge.reason);
      if (edge.evidence) showEvidence(details, edge.evidence);
      else paragraph(details, "責任の分解", "宣言上の接続です。下位契約のテスト成功は上位契約の直接検証にはなりません。");
      return;
    }
    const kind = selection.kind;
    const node = ["feature", "component"].includes(kind) ? entry() : graph.nodes.get(key(kind, selection.id)).node;
    details.append(element("p", kind === "test" ? classifications[node.classification] : kinds[kind], "eyebrow"), element("h2", title(node)));
    if (node.statement || node.description) paragraph(details, "説明", node.statement || node.description);
    sourceButton(details, node.source);
    sourceButton(details, node.record, "宣言ファイルを開く");
    if (kind === "feature") {
      list(details, "処理の流れ", node.flow, true);
      list(details, "限界", node.limitations);
      details.append(element("h3", "保証"));
      for (const item of node.guarantees) nodeLink(details, "guarantee", item);
    } else if (kind === "component") {
      list(details, "内部の制約・注記", node.notes);
      list(details, "外部の当事者", node.external.map(item => `${item.id} · ${item.title}`));
      for (const [heading, field] of [["所有する契約", "owner"], ["提供する契約", "provider"], ["利用する契約", "consumer"]]) {
        details.append(element("h3", heading));
        const matching = [...contracts.values()].filter(item => item[field] === node.id);
        if (!matching.length) details.append(element("p", "登録されていません。", "muted"));
        for (const item of matching) nodeLink(details, "contract", item);
      }
      for (const classification of ["internal", "infrastructure", "helper"]) {
        details.append(element("h3", classifications[classification]));
        const matching = [...tests.values()].filter(item => item.package === node.id && item.classification === classification);
        if (!matching.length) details.append(element("p", "登録されていません。", "muted"));
        for (const item of matching) nodeLink(details, "test", item);
      }
      for (const item of node.entrypoints) sourceButton(details, item.source, `起動点 · ${item.name}`);
    } else if (kind === "guarantee") {
      sourceButton(details, entry().record, "機能宣言を開く");
      details.append(element("h3", "フロントエンドの入口契約"));
      for (const id of node.roots) nodeLink(details, "contract", contracts.get(id));
    } else if (kind === "contract") {
      paragraph(details, "当事者", `${node.consumer} → ${node.provider}`);
      paragraph(details, "宣言所有者・インターフェース", `${node.owner} / ${node.interface}`);
      list(details, "前提条件", node.assumptions);
      list(details, "限界", node.limitations);
      if (node.gap) paragraph(details, "未検証事項", node.gap, "gap");
      for (const [field, label] of [["call", "呼出箇所"], ["implementation", "提供元の実装"], ["binding", "組立箇所"]]) sourceButton(details, node[field], label);
      details.append(element("h3", "利用する下位契約"));
      for (const dependency of node.requires) {
        nodeLink(details, "contract", contracts.get(dependency.contract));
        details.append(element("p", dependency.reason));
      }
      details.append(element("h3", "この契約を利用する上位契約"));
      for (const parent of contracts.values()) {
        const dependency = parent.requires.find(item => item.contract === node.id);
        if (!dependency) continue;
        nodeLink(details, "contract", parent);
        details.append(element("p", dependency.reason));
      }
      details.append(element("h3", "この契約を入口とする保証"));
      for (const feature of features.values()) {
        for (const guarantee of feature.guarantees) {
          if (!guarantee.roots.includes(node.id)) continue;
          const control = button(`${feature.title} · ${guarantee.id}`, () => select({ feature: feature.id, kind: "guarantee", id: guarantee.id }));
          control.dataset.parentFeature = feature.id;
          details.append(control);
        }
      }
      details.append(element("h3", "境界テストの直接の根拠"));
      details.append(element("p", "下位の根拠はこの契約へ繰り上げません。宣言と assertion の妥当性にはレビューが必要です。", "muted"));
      if (!evidence.get(node.id).length) details.append(element("p", "直接の根拠はありません。未検証事項を確認してください。", "gap"));
      for (const item of evidence.get(node.id)) {
        const edge = [...graph.edges.values()].find(edge => edge.evidence === item || edge.evidence?.test === item.test && edge.evidence.contract === node.id);
        const control = button(`${item.relationship} · ${title(tests.get(item.test))}`, () => select(request("edge", undefined, { edge: edge.id, context: node.id })));
        control.dataset.evidenceTo = item.test;
        details.append(control, element("p", item.reason, "evidence-reason"));
      }
    } else if (kind === "test") {
      paragraph(details, "Cargo テスト識別子", `${node.package} / ${node.target.kind} / ${node.target.name || "(lib)"} / ${node.name}`, "identity");
      if (node.classification === "boundary") {
        details.append(element("h3", "契約ごとの関係"));
        for (const item of node.semantics.evidence) {
          nodeLink(details, "contract", contracts.get(item.contract), undefined, `${item.relationship} · ${item.contract}`);
          showEvidence(details, item);
        }
      } else {
        paragraph(details, "所属コンポーネント", node.package);
        if (node.semantics.category) paragraph(details, "カテゴリ", node.semantics.category);
        if (node.semantics.reason) paragraph(details, "分類理由", node.semantics.reason);
        paragraph(details, "契約との関係", "この分類のテストには契約の根拠リンクを付けません。");
      }
    }
  }
  function applyZoom() {
    byId("graph").style.width = `${dimensions.width}px`;
    byId("graph").style.height = `${dimensions.height}px`;
    byId("graph").style.transform = `scale(${zoom})`;
    byId("graph-sizer").style.width = `${dimensions.width * zoom}px`;
    byId("graph-sizer").style.height = `${dimensions.height * zoom}px`;
    byId("zoom-level").textContent = `${Math.round(zoom * 100)}%`;
  }
  function openSource(location, opener) {
    const file = data.files[location.path];
    sourceOpener = opener;
    byId("source-title").textContent = `${location.path}:${location.line}`;
    byId("source-status").textContent = file.status;
    const remote = byId("source-remote");
    remote.replaceChildren();
    if (file.github_url) {
      const link = element("a", "同じソースを GitHub で開く");
      link.href = `${file.github_url}#L${location.line}`;
      link.target = "_blank";
      link.rel = "noopener noreferrer";
      remote.append(link);
    } else remote.append(element("span", "このソースの GitHub リンクはありません。", "muted"));
    const lines = byId("source-lines");
    lines.replaceChildren();
    file.content.split(/\r\n|\n|\r/).forEach((text, index) => {
      const line = element("div", undefined, `source-line${index + 1 === location.line ? " highlight" : ""}`);
      line.dataset.line = index + 1;
      if (index + 1 === location.line) line.setAttribute("aria-current", "true");
      line.append(element("span", String(index + 1), "line-number"), element("span", text, "line-content"));
      lines.append(line);
    });
    byId("source-dialog").showModal();
    lines.querySelector(".highlight")?.scrollIntoView({ block: "center" });
  }
  byId("source-close").addEventListener("click", () => byId("source-dialog").close());
  byId("source-dialog").addEventListener("close", () => sourceOpener?.focus());
  byId("feature-search").addEventListener("input", renderEntries);
  byId("feature-details").addEventListener("click", () => select(selection.feature ? { feature: selection.feature, kind: "feature", id: selection.feature } : { component: selection.component, kind: "component", id: selection.component }));
  byId("zoom-in").addEventListener("click", () => { zoom = Math.min(2, zoom + 0.1); applyZoom(); });
  byId("zoom-out").addEventListener("click", () => { zoom = Math.max(0.25, zoom - 0.1); applyZoom(); });
  byId("reset").addEventListener("click", () => { zoom = 1; applyZoom(); });
  byId("fit").addEventListener("click", () => {
    const viewport = byId("graph-viewport");
    zoom = Math.max(0.1, Math.min(1, (viewport.clientWidth - 12) / dimensions.width, (viewport.clientHeight - 12) / dimensions.height));
    applyZoom();
    viewport.scrollTo(0, 0);
  });
  window.addEventListener("hashchange", restore);
  window.addEventListener("popstate", restore);
  byId("revision").textContent = data.repository.revision ? `HEAD ${data.repository.revision}` : "Git revision unavailable · local snapshot";
  if (features.size || components.size) restore();
  else byId("details").append(element("p", "機能・コンポーネントの宣言がありません。"));
})();
