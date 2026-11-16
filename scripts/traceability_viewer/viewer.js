"use strict";
(() => {
  const data = JSON.parse(document.getElementById("viewer-data").textContent);
  const byId = id => document.getElementById(id);
  const relationships = {
    asserts: "事例を直接検証",
    exercises: "処理を通る",
    unreviewed: "所属のみ・未レビュー",
    structural: "宣言上の関連"
  };
  const kinds = { feature: "機能", guarantee: "保証", contract: "契約", test: "テスト" };
  const features = new Map(data.features.map(feature => {
    const graph = feature.graph;
    const nodes = new Map();
    const outgoing = new Map();
    const incoming = new Map();
    const edges = new Map();
    for (const [kind, group] of [["feature", [graph.feature]], ["guarantee", graph.guarantees], ["contract", graph.contracts], ["test", graph.tests]]) {
      for (const node of group) {
        nodes.set(node.id, { kind, node });
        outgoing.set(node.id, []);
        incoming.set(node.id, []);
      }
    }
    function addEdge(from, to, evidence) {
      const edge = { from, to, relationship: evidence?.relationship || "structural", reason: evidence?.reason };
      edges.set(`${from}/${to}`, edge);
      outgoing.get(from).push(edge);
      incoming.get(to).push(edge);
    }
    for (const guarantee of graph.guarantees) {
      addEdge(graph.feature.id, guarantee.id);
      for (const contract of guarantee.contracts) addEdge(guarantee.id, contract);
    }
    for (const contract of graph.contracts) {
      for (const evidence of contract.evidence) addEdge(contract.id, evidence.test, evidence);
    }
    return [graph.feature.id, { ...feature, nodes, outgoing, incoming, edges }];
  }));
  let selection;
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

  function title(node) {
    return node.title || (node.name ? node.name.split("::").at(-1) : node.id);
  }

  function currentFeature() {
    return features.get(selection.feature);
  }

  function notify(message) {
    byId("notice").textContent = message;
    byId("notice").hidden = !message;
  }

  function normalized(request) {
    const feature = features.get(request.feature);
    if (!feature) return null;
    if (request.kind === "edge") {
      const edge = feature.edges.get(`${request.from}/${request.to}`);
      if (!edge) return null;
      return { feature: request.feature, kind: "edge", from: edge.from, to: edge.to, context: feature.nodes.get(edge.from).kind === "contract" ? edge.from : null };
    }
    const entry = feature.nodes.get(request.id);
    if (!entry || entry.kind !== request.kind) return null;
    let context = null;
    if (entry.kind === "test") {
      const candidates = feature.incoming.get(request.id).map(edge => edge.from);
      if (request.context && !candidates.includes(request.context)) return null;
      context = request.context || candidates[0];
    } else if (entry.kind === "contract") {
      context = request.id;
    }
    return { feature: request.feature, kind: request.kind, id: request.id, context };
  }

  function hashFor(state) {
    const params = new URLSearchParams();
    for (const key of ["feature", "kind", "id", "from", "to", "context"]) {
      if (state[key]) params.set(key, state[key]);
    }
    return `#${params}`;
  }

  function select(request, historyMode = "push") {
    let next = normalized(request);
    if (!next) {
      const feature = features.get(request.feature) || features.values().next().value;
      next = normalized({ feature: feature.graph.feature.id, kind: "feature", id: feature.graph.feature.id });
      notify("URL の選択項目が見つからないため、機能の詳細を表示しています。");
    } else {
      notify("");
    }
    const changedFeature = selection?.feature !== next.feature;
    selection = next;
    if (historyMode === "push" && location.hash !== hashFor(next)) history.pushState(null, "", hashFor(next));
    if (historyMode === "replace") history.replaceState(null, "", hashFor(next));
    if (changedFeature) zoom = 1;
    renderFeatures();
    renderGraph();
    renderDetails();
  }

  function restore() {
    const params = new URLSearchParams(location.hash.slice(1));
    if (!location.hash) {
      const id = features.keys().next().value;
      select({ feature: id, kind: "feature", id }, "replace");
    } else {
      select(Object.fromEntries(params), "none");
    }
  }

  function renderFeatures() {
    const query = byId("feature-search").value.trim().toLocaleLowerCase();
    const container = byId("feature-list");
    container.replaceChildren();
    let count = 0;
    for (const [id, feature] of features) {
      const node = feature.graph.feature;
      if (!`${id} ${node.title}`.toLocaleLowerCase().includes(query)) continue;
      count++;
      const item = button(node.title, () => select({ feature: id, kind: "feature", id }), "feature-button");
      item.dataset.featureId = id;
      item.setAttribute("aria-current", String(selection.feature === id));
      item.append(element("small", id));
      container.append(item);
    }
    byId("feature-count").textContent = `${count} / ${features.size} 機能`;
    if (!count) container.append(element("p", "一致する機能がありません。", "muted"));
  }

  function renderGraph() {
    const feature = currentFeature();
    const graph = feature.graph;
    byId("feature-id").textContent = graph.feature.id;
    byId("feature-title").textContent = graph.feature.title;
    const canvas = byId("graph");
    const active = document.activeElement;
    const restoreFocus = canvas.contains(active);
    canvas.replaceChildren();
    const context = selection.context;
    const visibleTests = context ? feature.outgoing.get(context).map(edge => edge.to) : [];
    const contracts = [...new Set(graph.guarantees.flatMap(node => node.contracts))];
    const positions = new Map();
    contracts.forEach((id, row) => positions.set(id, { x: 340, y: 54 + row * 132 }));
    let lastGuaranteeY = -100;
    for (const guarantee of graph.guarantees) {
      const averageY = guarantee.contracts.reduce((sum, id) => sum + positions.get(id).y, 0) / guarantee.contracts.length;
      const y = Math.max(averageY, lastGuaranteeY + 132);
      positions.set(guarantee.id, { x: 20, y });
      lastGuaranteeY = y;
    }
    const testStart = Math.max(54, (positions.get(context)?.y || 54) - (visibleTests.length - 1) * 66);
    visibleTests.forEach((id, row) => positions.set(id, { x: 660, y: testStart + row * 132 }));
    dimensions = { width: context ? 920 : 600, height: Math.max(400, ...[...positions.values()].map(position => position.y + 140)) };
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("width", dimensions.width);
    svg.setAttribute("height", dimensions.height);
    svg.setAttribute("aria-hidden", "true");
    canvas.append(svg);
    for (const edge of feature.edges.values()) {
      if (!positions.has(edge.from) || !positions.has(edge.to)) continue;
      const from = positions.get(edge.from);
      const to = positions.get(edge.to);
      const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
      const startX = from.x + 240;
      const endX = to.x;
      path.setAttribute("d", `M ${startX} ${from.y + 44} C ${startX + 45} ${from.y + 44}, ${endX - 45} ${to.y + 44}, ${endX} ${to.y + 44}`);
      path.setAttribute("fill", "none");
      const related = edge.from === context || edge.to === context || (selection.kind === "edge" && selection.from === edge.from && selection.to === edge.to);
      path.setAttribute("stroke", related ? "#218f86" : edge.relationship === "unreviewed" ? "#b48a50" : "#c6d5e6");
      path.setAttribute("stroke-width", "1.8");
      path.style.pointerEvents = "stroke";
      path.style.cursor = "pointer";
      path.addEventListener("click", () => select({ feature: selection.feature, kind: "edge", from: edge.from, to: edge.to }));
      if (edge.relationship === "exercises") path.setAttribute("stroke-dasharray", "7 4");
      if (edge.relationship === "unreviewed") path.setAttribute("stroke-dasharray", "2 4");
      svg.append(path);
      const control = button(edge.relationship === "structural" ? "関連" : edge.relationship, () => select({ feature: selection.feature, kind: "edge", from: edge.from, to: edge.to }), "edge-button");
      control.dataset.from = edge.from;
      control.dataset.to = edge.to;
      control.dataset.relationship = edge.relationship;
      control.setAttribute("aria-label", `${edge.from} → ${edge.to}: ${relationships[edge.relationship]}`);
      control.setAttribute("aria-pressed", String(selection.kind === "edge" && selection.from === edge.from && selection.to === edge.to));
      control.style.left = `${startX + 10}px`;
      control.style.top = `${to.y + 10 + (feature.incoming.get(edge.to).findIndex(item => item.from === edge.from) % 4) * 22}px`;
      canvas.append(control);
    }
    for (const [id, position] of positions) {
      const { kind, node } = feature.nodes.get(id);
      const control = button(undefined, () => select({ feature: selection.feature, kind, id, context: kind === "test" ? context : null }), "graph-node");
      control.dataset.kind = kind;
      control.dataset.nodeId = id;
      control.setAttribute("aria-label", `${kinds[kind]}: ${title(node)}`);
      control.setAttribute("aria-pressed", String(selection.kind === kind && selection.id === id));
      control.style.left = `${position.x}px`;
      control.style.top = `${position.y}px`;
      control.append(element("span", kinds[kind], "node-kind"), element("span", title(node), "node-title"));
      control.append(element("span", node.statement || node.package, "node-caption"));
      if (node.gap) control.append(element("span", "未検証事項あり", "gap-mark"));
      canvas.append(control);
    }
    applyZoom();
    if (restoreFocus) canvas.querySelector('[aria-pressed="true"]')?.focus({ preventScroll: true });
    if (["contract", "test"].includes(selection.kind)) canvas.querySelector('.graph-node[aria-pressed="true"]')?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }

  function paragraph(container, heading, text, className) {
    container.append(element("h3", heading), element("p", text, className));
  }

  function list(container, heading, values, ordered = false) {
    container.append(element("h3", heading));
    const result = element(ordered ? "ol" : "ul");
    for (const value of values) result.append(element("li", value));
    container.append(result);
  }

  function nodeLink(container, id, context) {
    const entry = currentFeature().nodes.get(id);
    const control = button(`${kinds[entry.kind]} · ${title(entry.node)}`, () => select({ feature: selection.feature, kind: entry.kind, id, context }));
    control.dataset.relatedId = id;
    container.append(control);
  }

  function sourceButton(container, location, label = "ソースを開く") {
    const control = button(label, () => openSource(location, control));
    control.dataset.sourceOpen = "true";
    control.dataset.sourcePath = location.path;
    control.dataset.sourceLine = location.line;
    container.append(control);
  }

  function renderDetails() {
    const feature = currentFeature();
    const graph = feature.graph;
    const details = byId("details");
    details.replaceChildren();
    if (selection.kind === "edge") {
      const edge = feature.edges.get(`${selection.from}/${selection.to}`);
      details.append(element("p", "RELATIONSHIP", "eyebrow"), element("h2", relationships[edge.relationship]));
      details.append(element("p", `${edge.from} → ${edge.to}`, "identity"));
      paragraph(details, "関係", edge.relationship === "structural" ? "宣言に記載された関連です。検証の根拠を示す関係ではありません。" : edge.relationship);
      nodeLink(details, edge.from);
      nodeLink(details, edge.to, selection.context);
      if (edge.reason) paragraph(details, "根拠", edge.reason);
      return;
    }
    const { kind, node } = feature.nodes.get(selection.id);
    details.append(element("p", kinds[kind], "eyebrow"), element("h2", title(node)));
    if (node.statement) paragraph(details, "説明", node.statement);
    if (feature.locations[node.id]) sourceButton(details, feature.locations[node.id]);
    if (kind === "feature") {
      sourceButton(details, { path: feature.record, line: 1 }, "機能宣言を開く");
      list(details, "処理の流れ", node.flow, true);
      list(details, "限界", graph.limitations);
      details.append(element("h3", "保証"));
      for (const guarantee of graph.guarantees) nodeLink(details, guarantee.id);
    } else if (kind === "guarantee") {
      details.append(element("p", "保証の宣言は機能定義にあります。", "muted"));
      sourceButton(details, { path: feature.record, line: 1 }, "機能宣言を開く");
      details.append(element("h3", "関連する契約"));
      for (const contract of node.contracts) nodeLink(details, contract);
    } else if (kind === "contract") {
      if (node.gap) paragraph(details, "未検証事項", node.gap, "gap");
      details.append(element("h3", "関連する保証"));
      for (const edge of feature.incoming.get(node.id)) nodeLink(details, edge.from);
      details.append(element("h3", "テストとの関係"));
      if (!node.evidence.length) details.append(element("p", "テストの根拠は登録されていません。未検証事項を確認してください。", "gap"));
      for (const edge of feature.outgoing.get(node.id)) {
        const control = button(`${edge.relationship} · ${title(feature.nodes.get(edge.to).node)}`, () => select({ feature: selection.feature, kind: "edge", from: edge.from, to: edge.to }));
        control.dataset.evidenceTo = edge.to;
        details.append(control, element("p", edge.reason, "evidence-reason"));
      }
    } else if (kind === "test") {
      paragraph(details, "Cargo テスト識別子", `${node.package} / ${node.target.kind} / ${node.target.name || "(lib)"} / ${node.name}`, "identity");
      paragraph(details, "検証範囲", node.scope);
      details.append(element("h3", "契約ごとの関係"));
      for (const edge of feature.incoming.get(node.id)) {
        nodeLink(details, edge.from);
        const control = button(`${edge.relationship} · ${relationships[edge.relationship]}`, () => select({ feature: selection.feature, kind: "edge", from: edge.from, to: edge.to }));
        details.append(control, element("p", edge.reason));
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
    } else {
      remote.append(element("span", "このソースの GitHub リンクはありません。", "muted"));
    }
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
  byId("feature-search").addEventListener("input", renderFeatures);
  byId("feature-details").addEventListener("click", () => select({ feature: selection.feature, kind: "feature", id: selection.feature }));
  byId("zoom-in").addEventListener("click", () => { zoom = Math.min(2, zoom + 0.1); applyZoom(); });
  byId("zoom-out").addEventListener("click", () => { zoom = Math.max(0.25, zoom - 0.1); applyZoom(); });
  byId("reset").addEventListener("click", () => { zoom = 1; applyZoom(); });
  byId("fit").addEventListener("click", () => {
    const viewport = byId("graph-viewport");
    zoom = Math.min(1, (viewport.clientWidth - 12) / dimensions.width, (viewport.clientHeight - 12) / dimensions.height);
    applyZoom();
    viewport.scrollTo(0, 0);
  });
  window.addEventListener("hashchange", restore);
  window.addEventListener("popstate", restore);
  byId("revision").textContent = data.repository.revision ? `HEAD ${data.repository.revision}` : "Git revision unavailable · local snapshot";
  restore();
})();
