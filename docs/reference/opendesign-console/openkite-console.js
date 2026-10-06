(function(){
  const $ = (s, r) => (r || document).querySelector(s);
  const $$ = (s, r) => Array.from((r || document).querySelectorAll(s));
  const sidebar = $('#sidebar');
  const sidebarBackdrop = $('#sidebarBackdrop');
  const menuToggle = $('#menuToggle');
  const main = $('#main');
  const toast = $('#toast');
  const views = $$('.view');
  const navItems = $$('.nav-item[data-view]');
  const bottomTabs = $$('.bottom-tab[data-view]');
  const logPanel = $('#logPanel');
  const logTab = $('#logTab');
  const inspector = $('#inspector');
  const inspectorScrim = $('#inspectorScrim');

  let toastTimer;
  function showToast(msg){
    toast.textContent = msg;
    toast.classList.add('show');
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => toast.classList.remove('show'), 2200);
  }

  function setView(viewId){
    views.forEach(v => v.classList.toggle('active', v.id === 'view-' + viewId));
    navItems.forEach(b => b.classList.toggle('active', b.dataset.view === viewId));
    bottomTabs.forEach(b => b.classList.toggle('active', b.dataset.view === viewId));
    const isArgo = viewId === 'argocd';
    $('#crumbSection').textContent = isArgo ? 'Argo CD' : 'Workloads';
    $('#crumbCurrent').textContent = isArgo ? 'Applications' : 'Pods';
    closeSidebar();
    logPanel.classList.remove('open');
    logTab.setAttribute('aria-expanded', 'false');
    main.scrollTop = 0;
  }

  navItems.forEach(btn => {
    btn.addEventListener('click', () => {
      if (btn.dataset.view) setView(btn.dataset.view);
    });
  });

  bottomTabs.forEach(btn => {
    btn.addEventListener('click', () => {
      if (btn.dataset.view) setView(btn.dataset.view);
    });
  });

  function openSidebar(){
    sidebar.classList.add('open');
    sidebarBackdrop.classList.add('show');
    menuToggle.setAttribute('aria-expanded', 'true');
  }
  function closeSidebar(){
    sidebar.classList.remove('open');
    sidebarBackdrop.classList.remove('show');
    menuToggle.setAttribute('aria-expanded', 'false');
  }
  menuToggle.addEventListener('click', openSidebar);
  sidebarBackdrop.addEventListener('click', closeSidebar);
  $('#menuTab').addEventListener('click', openSidebar);

  logTab.addEventListener('click', () => {
    logPanel.classList.toggle('open');
    const open = logPanel.classList.contains('open');
    logTab.setAttribute('aria-expanded', String(open));
  });
  $('#logHandle').addEventListener('click', () => {
    logPanel.classList.remove('open');
    logTab.setAttribute('aria-expanded', 'false');
  });

  $$('.nav-item[data-disabled], [data-toast]').forEach(el => {
    el.addEventListener('click', () => {
      if (el.dataset.toast) showToast(el.dataset.toast);
    });
  });

  $$('[data-refresh]').forEach(btn => {
    btn.addEventListener('click', () => {
      const icon = btn.querySelector('.icon');
      btn.classList.add('refreshing');
      if (icon) icon.style.animation = 'spin 0.7s linear infinite';
      showToast('Resources refreshed');
      setTimeout(() => {
        btn.classList.remove('refreshing');
        if (icon) icon.style.animation = '';
      }, 800);
    });
  });

  // Namespace + text filtering
  const chips = $$('.chip[data-ns]');
  const rows = $$('#podRows tr');
  const podSearch = $('#podSearch');
  function applyFilters(){
    const activeNs = ($('.chip.active[data-ns]') || {}).dataset?.ns || 'all';
    const q = (podSearch.value || '').trim().toLowerCase();
    let visible = 0;
    rows.forEach(row => {
      const nsMatch = activeNs === 'all' || row.dataset.ns === activeNs;
      const nameMatch = !q || row.dataset.name.toLowerCase().includes(q);
      const show = nsMatch && nameMatch;
      row.hidden = !show;
      if (show) visible++;
    });
    $('#rowCount').textContent = `Showing ${visible} of ${rows.length} pods`;
  }
  chips.forEach(chip => chip.addEventListener('click', () => {
    chips.forEach(c => c.classList.remove('active'));
    chip.classList.add('active');
    applyFilters();
  }));
  podSearch.addEventListener('input', applyFilters);

  // Row selection + inspector
  function openInspector(row){
    $('#inspectorName').textContent = row.dataset.name;
    $('#inspectorNs').textContent = row.dataset.ns;
    $('#inspectorController').textContent = row.dataset.controller;
    $('#inspectorNode').textContent = row.dataset.node;
    $('#inspectorQos').textContent = row.dataset.qos;
    $('#inspectorAge').textContent = row.dataset.age;
    $('#inspectorStatus').textContent = row.dataset.status;
    inspector.classList.add('open');
    inspectorScrim.classList.add('show');
    rows.forEach(r => r.classList.remove('selected'));
    row.classList.add('selected');
  }
  rows.forEach(row => {
    row.addEventListener('click', () => openInspector(row));
    row.addEventListener('keydown', e => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); openInspector(row); }
    });
    row.setAttribute('tabindex', '0');
  });
  function closeInspector(){
    inspector.classList.remove('open');
    inspectorScrim.classList.remove('show');
    rows.forEach(r => r.classList.remove('selected'));
  }
  $('#closeInspector').addEventListener('click', closeInspector);
  inspectorScrim.addEventListener('click', closeInspector);

  // Log controls
  const pauseLogs = $('#pauseLogs');
  pauseLogs.addEventListener('click', () => {
    const paused = logPanel.classList.toggle('paused');
    pauseLogs.setAttribute('aria-pressed', String(paused));
    showToast(paused ? 'Log stream paused' : 'Log stream resumed');
  });
  $('#clearLogs').addEventListener('click', () => {
    $('#logBody').innerHTML = '<div class="log-line"><span class="log-time">--:--:--.---</span><span class="log-level">INFO</span><span class="log-msg">log buffer cleared</span></div>';
    showToast('Log buffer cleared');
  });

  // Argo card swipe + menu actions
  function setSwipe(card, swiped){
    card.classList.toggle('swiped', swiped);
  }
  $$('.app-card').forEach(card => {
    const menuBtn = $('.card-menu-btn', card);
    menuBtn.addEventListener('click', e => {
      e.stopPropagation();
      setSwipe(card, !card.classList.contains('swiped'));
    });
    card.addEventListener('click', () => {
      if (card.classList.contains('swiped')) setSwipe(card, false);
    });
    let startX = 0, startY = 0, tracking = false;
    card.addEventListener('pointerdown', e => {
      startX = e.clientX; startY = e.clientY; tracking = true;
    });
    card.addEventListener('pointerup', e => {
      if (!tracking) return;
      tracking = false;
      const dx = e.clientX - startX;
      const dy = e.clientY - startY;
      if (Math.abs(dx) > 44 && Math.abs(dx) > Math.abs(dy)) {
        setSwipe(card, dx < 0);
      }
    });
    card.addEventListener('pointercancel', () => { tracking = false; });
  });

  // Pull-to-refresh on the main scroll area
  let pullStartY = null;
  main.addEventListener('touchstart', e => {
    if (main.scrollTop <= 0) pullStartY = e.touches[0].clientY;
  }, {passive:true});
  main.addEventListener('touchmove', e => {
    if (pullStartY === null) return;
    const dy = e.touches[0].clientY - pullStartY;
    if (dy > 24 && main.scrollTop <= 0) {
      $('#pullIndicator').classList.add('show');
      $('#pullIndicator').textContent = dy > 90 ? 'Release to refresh' : 'Pull to refresh';
    }
  }, {passive:true});
  main.addEventListener('touchend', e => {
    const indicator = $('#pullIndicator');
    if (indicator.classList.contains('show')) {
      indicator.classList.remove('show');
      indicator.innerHTML = '<span class="spinner"></span>Release to refresh';
      showToast('Resource list refreshed');
      setTimeout(() => {
        indicator.textContent = '';
      }, 600);
    }
    pullStartY = null;
  });

  // Close side drawer on Escape
  document.addEventListener('keydown', e => {
    if (e.key === 'Escape') {
      closeSidebar();
      closeInspector();
      logPanel.classList.remove('open');
      logTab.setAttribute('aria-expanded', 'false');
    }
  });
})();
