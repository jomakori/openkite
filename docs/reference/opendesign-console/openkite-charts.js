/* OpenKite charts — dependency-free SVG charts, sparklines and gauges with deterministic demo metrics. */
(function(){
  const esc = s => String(s).replace(/[&<>"]/g, m => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}[m]));
  const RANGES = {'1h':{n:60, step:60}, '6h':{n:72, step:300}, '24h':{n:96, step:900}, '7d':{n:84, step:7200}};
  const END = Date.UTC(2025, 5, 12, 10, 42, 0);

  function rng(seed){ let s = 11; for (const c of String(seed)) s = (s * 31 + c.charCodeAt(0)) >>> 0; return () => { s = (s * 1664525 + 1013904223) >>> 0; return s / 4294967296; }; }
  function series(seed, n, base, amp, noise){
    const r = rng(seed), ph = r() * 6.28, f = 1 + Math.floor(r() * 3), out = [];
    let drift = 0;
    for (let i = 0; i < n; i++) {
      drift = drift * 0.85 + (r() - 0.5) * noise * 0.5;
      out.push(Math.max(0, base + amp * Math.sin(i / n * Math.PI * 2 * f + ph) + drift + (r() - 0.5) * noise * 0.35));
    }
    return out;
  }

  const FMT = {
    cpu:v => v >= 1000 ? (v / 1000).toFixed(2) + ' cores' : Math.round(v) + 'm',
    mem:v => v >= 1024 ? (v / 1024).toFixed(1) + ' GiB' : Math.round(v) + ' MiB',
    bps:v => v >= 1024 ? (v / 1024).toFixed(1) + ' MiB/s' : Math.round(v) + ' KiB/s',
    pct:v => Math.round(v) + '%'
  };
  const SHORT = {
    cpu:v => v >= 1000 ? +(v / 1000).toFixed(1) + 'c' : Math.round(v) + 'm',
    mem:v => v >= 1024 ? +(v / 1024).toFixed(1) + 'G' : Math.round(v) + 'M',
    bps:v => v >= 1024 ? +(v / 1024).toFixed(1) + 'M' : Math.round(v) + 'K',
    pct:v => Math.round(v) + '%'
  };

  function timeAt(range, i, n, long){
    const R = RANGES[range], t = new Date(END - (n - 1 - i) * R.step * 1000), p = x => String(x).padStart(2, '0');
    const hm = p(t.getUTCHours()) + ':' + p(t.getUTCMinutes());
    if (range === '7d') return ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun'][5] + ' ' + t.getUTCDate() + (long ? ' ' + hm : '');
    return hm;
  }

  /* metrics for a resource: scale = number of pod-equivalents it represents */
  function metrics(seed, scale, range, o){
    o = o || {};
    const n = RANGES[range].n, sc = Math.max(1, scale), r = rng(seed + 'fs');
    const s = (k, base, amp, noise) => series(seed + k + range, n, base * sc, amp * sc, noise * sc);
    return {
      n, range,
      cpu:{data:s('c', 140, 60, 70), request:(o.cpuReq || 100 * sc), limit:(o.cpuLimit || 500 * sc)},
      mem:{data:s('m', 270, 30, 30), request:(o.memReq || 128 * sc), limit:(o.memLimit || 512 * sc)},
      rx:s('rx', 70, 45, 80), tx:s('tx', 35, 22, 50),
      fr:s('fr', 14, 10, 28), fw:s('fw', 42, 30, 60),
      fsPct:Math.round(18 + r() * 52)
    };
  }
  const last = a => a[a.length - 1];
  const sumSeries = list => list.reduce((acc, a) => acc ? acc.map((v, i) => v + a[i]) : a.slice(), null);

  /* ---------- line / area chart */
  const registry = {};
  let uid = 0;
  function niceMax(v){
    if (v <= 0) return 1;
    const e = Math.pow(10, Math.floor(Math.log10(v))), f = v / e;
    return (f <= 1 ? 1 : f <= 2 ? 2 : f <= 5 ? 5 : 10) * e;
  }
  function chart(title, ser, o){
    o = o || {};
    const id = 'ch' + (++uid), n = ser[0].data.length, fmt = FMT[o.unit] || String, sh = SHORT[o.unit] || String;
    const W = 320, H = o.h || 124, L = 40, R = 8, T = 8, B = 18;
    const peak = Math.max.apply(null, ser.map(s => Math.max.apply(null, s.data)).concat([o.limit || 0, o.request || 0]));
    const max = niceMax(peak * 1.12);
    const x = i => L + i / (n - 1) * (W - L - R), y = v => T + (1 - v / max) * (H - T - B);
    let g = '';
    [0, 0.5, 1].forEach(f => {
      const yy = y(max * f);
      g += '<line class="grid" x1="' + L + '" x2="' + (W - R) + '" y1="' + yy + '" y2="' + yy + '"/><text class="axis" x="' + (L - 5) + '" y="' + (yy + 3) + '" text-anchor="end">' + esc(sh(max * f)) + '</text>';
    });
    [0, 0.5, 1].forEach((f, k) => {
      const i = Math.round((n - 1) * f);
      g += '<text class="axis" x="' + x(i) + '" y="' + (H - 4) + '" text-anchor="' + (k === 0 ? 'start' : k === 2 ? 'end' : 'middle') + '">' + esc(timeAt(o.range || '1h', i, n)) + '</text>';
    });
    let refs = '';
    if (o.limit) refs += '<line class="ref lim" x1="' + L + '" x2="' + (W - R) + '" y1="' + y(o.limit) + '" y2="' + y(o.limit) + '"/>';
    if (o.request) refs += '<line class="ref req" x1="' + L + '" x2="' + (W - R) + '" y1="' + y(o.request) + '" y2="' + y(o.request) + '"/>';
    let paths = '';
    ser.forEach(s => {
      const pts = s.data.map((v, i) => x(i).toFixed(1) + ' ' + y(v).toFixed(1));
      paths += '<path class="area" style="fill:' + s.color + '" d="M' + pts.join(' L') + ' L' + x(n - 1).toFixed(1) + ' ' + y(0) + ' L' + L + ' ' + y(0) + ' Z"/>' +
        '<path class="line" style="stroke:' + s.color + '" d="M' + pts.join(' L') + '"/>';
    });
    registry[id] = {ser, o, n, W, L, R, T, B, H, max, fmt};
    const legend = ser.map(s => '<span class="lg"><i style="background:' + s.color + '"></i>' + esc(s.name) + '<b>' + esc(fmt(last(s.data))) + '</b></span>').join('') +
      (o.request ? '<span class="lg ref-lg"><i class="dash req"></i>request</span>' : '') + (o.limit ? '<span class="lg ref-lg"><i class="dash lim"></i>limit</span>' : '');
    const alt = title + ': ' + ser.map(s => s.name + ' now ' + fmt(last(s.data))).join(', ');
    return '<figure class="chart-card' + (o.metric ? ' clickable' : '') + '"' + (o.metric ? ' data-metric="' + o.metric + '" data-scope="' + esc(o.scope || '') + '" title="Click for metric details"' : '') + '><figcaption><span class="ct">' + esc(title) + '</span>' + (o.sub ? '<span class="cs">' + esc(o.sub) + '</span>' : '') + (o.metric ? '<span class="cd" aria-hidden="true">Details ›</span>' : '') + '</figcaption>' +
      '<div class="chart-wrap"><svg class="chart-svg" data-chart="' + id + '" viewBox="0 0 ' + W + ' ' + H + '" role="img" aria-label="' + esc(alt + (o.metric ? '. Press Enter for details' : '')) + '" tabindex="0">' + g + refs + paths +
      '<line class="cross" x1="0" x2="0" y1="' + T + '" y2="' + (H - B) + '" hidden/>' + ser.map((s, k) => '<circle class="dotm" data-k="' + k + '" r="3" style="fill:' + s.color + '" hidden/>').join('') + '</svg>' +
      '<div class="chart-tip" hidden></div></div><div class="legend">' + legend + '</div></figure>';
  }

  function spark(data, color){
    const n = data.length, W = 72, H = 22, max = Math.max.apply(null, data) * 1.1 || 1;
    const pts = data.map((v, i) => (i / (n - 1) * W).toFixed(1) + ' ' + (H - 2 - v / max * (H - 4)).toFixed(1));
    return '<svg class="spark" viewBox="0 0 ' + W + ' ' + H + '" aria-hidden="true"><path style="fill:' + color + '" class="area" d="M' + pts.join(' L') + ' L' + W + ' ' + H + ' L0 ' + H + ' Z"/><path class="line" style="stroke:' + color + '" d="M' + pts.join(' L') + '"/></svg>';
  }

  function gauge(pct, label, sub, color){
    pct = Math.max(0, Math.min(100, pct));
    const tone = pct >= 85 ? 'var(--danger)' : pct >= 70 ? 'var(--warn)' : (color || 'var(--accent)');
    return '<div class="gauge" role="img" aria-label="' + esc(label) + ' ' + Math.round(pct) + ' percent"><svg viewBox="0 0 42 42" aria-hidden="true"><circle class="g-bg" cx="21" cy="21" r="15.9155"/><circle class="g-fg" cx="21" cy="21" r="15.9155" style="stroke:' + tone + '" stroke-dasharray="' + pct.toFixed(1) + ' ' + (100 - pct).toFixed(1) + '" transform="rotate(-90 21 21)"/></svg>' +
      '<div class="g-val">' + Math.round(pct) + '<small>%</small></div><div class="g-label">' + esc(label) + '</div>' + (sub ? '<div class="g-sub">' + esc(sub) + '</div>' : '') + '</div>';
  }

  function rangeTabs(cur){
    return '<div class="range-tabs" role="group" aria-label="Time range">' + Object.keys(RANGES).map(k => '<button type="button" data-range="' + k + '" aria-pressed="' + (k === cur) + '">' + k + '</button>').join('') + '</div>';
  }

  /* ---------- tooltips (mouse hover and touch tap) */
  function showAt(svg, clientX){
    const c = registry[svg.dataset.chart];
    if (!c) return;
    const rect = svg.getBoundingClientRect();
    if (!rect.width) return;
    const vx = (clientX - rect.left) / rect.width * c.W;
    const i = Math.max(0, Math.min(c.n - 1, Math.round((vx - c.L) / (c.W - c.L - c.R) * (c.n - 1))));
    const xx = c.L + i / (c.n - 1) * (c.W - c.L - c.R);
    const cross = svg.querySelector('.cross');
    cross.setAttribute('x1', xx); cross.setAttribute('x2', xx); cross.removeAttribute('hidden');
    svg.querySelectorAll('.dotm').forEach((d, k) => {
      d.setAttribute('cx', xx); d.setAttribute('cy', c.T + (1 - c.ser[k].data[i] / c.max) * (c.H - c.T - c.B)); d.removeAttribute('hidden');
    });
    const tip = svg.parentNode.querySelector('.chart-tip');
    tip.innerHTML = '<b>' + esc(timeAt(c.o.range || '1h', i, c.n, true)) + '</b>' + c.ser.map(s => '<span><i style="background:' + s.color + '"></i>' + esc(s.name) + ' <em>' + esc(c.fmt(s.data[i])) + '</em></span>').join('');
    tip.removeAttribute('hidden');
    const wr = svg.parentNode.getBoundingClientRect(), w = tip.offsetWidth, cx = xx / c.W * rect.width + (rect.left - wr.left);   // the svg may be shifted inside its wrapper on small screens
    tip.style.left = Math.max(0, Math.min(wr.width - w, cx - w / 2)) + 'px';
  }
  function hideAt(svg){
    svg.querySelector('.cross').setAttribute('hidden', '');
    svg.querySelectorAll('.dotm').forEach(d => d.setAttribute('hidden', ''));
    svg.parentNode.querySelector('.chart-tip').setAttribute('hidden', '');
  }
  document.addEventListener('pointermove', e => { const s = e.target.closest && e.target.closest('.chart-svg'); if (s) showAt(s, e.clientX); });
  document.addEventListener('keydown', e => {
    const s = e.target.closest && e.target.closest('.chart-svg');
    if (s && (e.key === 'Enter' || e.key === ' ')) { const f = s.closest('.chart-card[data-metric]'); if (f) { e.preventDefault(); f.click(); } }
  });
  document.addEventListener('pointerdown', e => {
    const s = e.target.closest && e.target.closest('.chart-svg');
    document.querySelectorAll('.chart-svg').forEach(o => { if (o !== s) hideAt(o); });
    if (s) showAt(s, e.clientX);
  });
  document.addEventListener('pointerout', e => {
    const s = e.target.closest && e.target.closest('.chart-svg');
    if (s && e.pointerType === 'mouse' && !(e.relatedTarget && s.contains(e.relatedTarget))) hideAt(s);
  });

  function stats(data, range){
    const n = data.length, sorted = data.slice().sort((a, b) => a - b), sum = data.reduce((a, v) => a + v, 0);
    let maxI = 0; data.forEach((v, i) => { if (v > data[maxI]) maxI = i; });
    return {now:data[n - 1], avg:sum / n, min:sorted[0], max:data[maxI], p95:sorted[Math.min(n - 1, Math.floor(n * 0.95))], maxAt:timeAt(range, maxI, n, true)};
  }
  window.OKCharts = {RANGES, FMT, rng, series, metrics, sumSeries, chart, spark, gauge, rangeTabs, last, stats, timeAt};
})();
