const API_BASE = window.ZYVOR_API_URL || '/api/v1';

const WIZARD_STEPS = [
  { id: 'ingest', label: 'Ingest', hint: 'Upload disk' },
  { id: 'assure', label: 'Assure', hint: 'Boot score' },
  { id: 'plan', label: 'Plan', hint: 'Migration path' },
  { id: 'launch', label: 'Launch', hint: 'KubeVirt YAML' },
];

const WIZARD_ACTIONS = {
  assure: { action: 'doctor', label: 'Run Doctor' },
  plan: { action: 'migration-plan', label: 'Run Migrate Plan' },
  launch: { action: 'provision', label: 'Generate YAML' },
};

const state = {
  vms: [],
  selectedVm: null,
  activeJob: null,
  pollTimer: null,
  lastYaml: null,
  lastFailedAction: null,
  wizardChain: false,
  wizard: { step: 'ingest', completed: new Set() },
  vmCache: {},
  lastBriefing: null,
  lastJobId: null,
  inspectionMode: localStorage.getItem('zyvor.inspectMode') || 'offline',
  agentProxyUrl: localStorage.getItem('zyvor.agentProxyUrl') || '',
  agentReachable: false,
  fleetMode: localStorage.getItem('zyvor.fleetMode') || 'disks',
  clusterVms: [],
  clusterNamespaces: [],
  selectedClusterVm: null,
  lastClusterGuestInfo: null,
  lastClusterGuestIntel: null,
  lastGuestControlStatus: null,
  lastGuestDoctor: null,
  lastClusterBootInspect: null,
  lastClusterInspect: null,
  lastClusterBriefing: null,
  uiConfig: null,
  fleetFilters: {
    namespace: '',
    search: '',
    phase: '',
  },
  clusterLastSync: null,
  clusterFleetLoading: false,
  vmtoolsCoverage: null,
  vmtoolsPolicy: null,
  vmtoolsBundle: null,
  pendingGuestActions: [],
  guestActionAudit: [],
  serverStorage: { rootId: 0, path: '', roots: [] },
};

const AGENT_QUICK_RPC = [
  { label: 'Ping', method: 'guestkit.ping', params: {} },
  { label: 'Version', method: 'guestkit.getVersion', params: {} },
  { label: 'Capabilities', method: 'guestkit.getCapabilities', params: {} },
  { label: 'Guest health', method: 'guestkit.getGuestHealth', params: {} },
  { label: 'Guest info', method: 'guestkit.getGuestInfo', params: {} },
  { label: 'Processes', method: 'guestkit.getProcesses', params: {} },
  { label: 'Boot analysis', method: 'guestkit.getBootAnalysis', params: {} },
  { label: 'Metrics', method: 'guestkit.getMetrics', params: {} },
  { label: 'Filesystem', method: 'guestkit.getFilesystem', params: {} },
  { label: 'Whoami', method: 'guestkit.exec', params: { command: ['whoami'] } },
];

const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => document.querySelectorAll(sel);

function guestEnvelope(res) {
  if (res && typeof res.ok === 'boolean') return res;
  if (res?.data && typeof res.data.ok === 'boolean') return res.data;
  return { ok: res?.success ?? true, data: res?.data ?? res, transport: null, controlState: null, warnings: [], recommendedActions: [] };
}

function controlStateLabel(state) {
  const map = {
    full_agent: 'CONNECTED — FULL AGENT',
    airgap_live: 'CONNECTED VIA QGA EXEC',
    qga_only: 'QGA ONLY',
    disk_only: 'DISK ONLY',
    console_only: 'CONSOLE ONLY',
    blind_vm: 'BLIND VM',
  };
  return map[state] || (state || 'UNKNOWN').replace(/_/g, ' ').toUpperCase();
}

function controlStateClass(state) {
  if (state === 'full_agent' || state === 'airgap_live') return 'ok';
  if (state === 'qga_only' || state === 'disk_only') return 'warn';
  return 'err';
}

async function api(path, options = {}) {
  const headers = window.GuestKitAuth?.authHeaders(options.headers || {}) || (options.headers || {});
  const res = await fetch(`${API_BASE}${path}`, { ...options, headers });
  const data = await res.json().catch(() => ({}));
  if (res.status === 401 && window.GuestKitAuth) {
    const cfg = await window.GuestKitAuth.fetchAuthConfig().catch(() => ({ auth_enabled: false }));
    if (cfg.auth_enabled) {
      window.GuestKitAuth.redirectToLogin();
      throw new Error('Authentication required');
    }
  }
  if (!res.ok) throw new Error(data.message || data.error || res.statusText);
  return data;
}

function getTarget() {
  return $('#targetSelect')?.value || 'kubevirt';
}

function fmtBytes(n) {
  if (!n) return '0 B';
  const u = ['B', 'KB', 'MB', 'GB', 'TB'];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
  return `${v.toFixed(i ? 1 : 0)} ${u[i]}`;
}

const SMOKE_MAX_BYTES = 512 * 1024;

function isSmokeDisk(vm) {
  if (!vm) return false;
  const name = (vm.name || '').toLowerCase();
  if (/smoke|placeholder|empty-disk|test-shell/.test(name)) return true;
  return (vm.size_bytes || 0) > 0 && vm.size_bytes < SMOKE_MAX_BYTES;
}

function isShadowDisk(vm) {
  if (!vm) return false;
  if ((vm.size_bytes || 0) === 0) return true;
  const name = (vm.name || '').toLowerCase();
  return /\(cluster (doctor|inspect)\)|cluster-shadow/.test(name);
}

function isFleetDisk(vm) {
  return vm && !isSmokeDisk(vm) && !isShadowDisk(vm);
}

function fleetRank(vm) {
  const cache = getVmCache(vm.id);
  if (isSmokeDisk(vm)) return 100;
  if (cache.status === 'failed' && !cache.bootScore) return 85;
  if (cache.status === 'ready') return 0;
  if (cache.bootScore != null) return 10 + (100 - Math.min(cache.bootScore, 100));
  if (cache.status === 'analyzed') return 20;
  return 50;
}

function pickBestVm(vms) {
  const candidates = (vms || []).filter(isFleetDisk);
  const pool = candidates.length ? candidates : (vms || []).filter((v) => !isShadowDisk(v));
  return [...pool].sort((a, b) => fleetRank(a) - fleetRank(b))[0] || null;
}

function humanizeJobError(err, vm) {
  if (!err) return err;
  const lower = String(err).toLowerCase();
  if (lower.includes('no operating system')) {
    if (isSmokeDisk(vm)) {
      return 'This disk is a tiny smoke/placeholder image with no guest OS. Select a real cloud image (Cirros or Ubuntu minimal) or upload one in Ingest.';
    }
    return 'No guest OS detected in this disk. Use a cloud image with a real root filesystem (e.g. Ubuntu cloud-img, Cirros).';
  }
  return String(err).replace(/^Execution error:\s*/i, '');
}

function fmtTime(d = new Date()) {
  return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

function toast(msg, type = 'ok') {
  // Routed through the guestkit-ux toast bus (rich toasts: icon, dismiss, dedup).
  window.dispatchEvent(new CustomEvent('gk:toast', { detail: { msg, type } }));
}

function feed(msg, type = '') {
  const ul = $('#activityFeed');
  if (!ul) return;
  const li = document.createElement('li');
  li.className = type;
  li.innerHTML = `${msg}<span class="time">${fmtTime()}</span>`;
  ul.prepend(li);
  while (ul.children.length > 40) ul.lastChild.remove();
}

function setHealth(ok) {
  const dot = $('#healthDot');
  const label = $('#healthLabel');
  const stripLabel = $('#healthLabelStrip');
  const stripDot = $('#statusDotApi');
  dot.className = 'pulse-dot ' + (ok ? 'ok' : 'err');
  label.textContent = ok ? 'API online' : 'API offline';
  if (stripLabel) stripLabel.textContent = ok ? 'online' : 'offline';
  if (stripDot) stripDot.className = 'status-dot ' + (ok ? 'ok' : 'err');
}

async function checkHealth() {
  try {
    await api('/health');
    setHealth(true);
  } catch {
    setHealth(false);
  }
}

function getVmCache(vmId) {
  return state.vmCache[vmId] || { status: 'imported' };
}

function updateVmCache(vmId, patch) {
  state.vmCache[vmId] = { ...getVmCache(vmId), ...patch };
  renderFleet();
  if (state.selectedVm?.id === vmId) {
    window.GuestKitNebula?.renderAllNebula?.(state.selectedVm, state.vmCache[vmId]);
  }
}

function vmStatusLabel(cache, vm) {
  if (isSmokeDisk(vm)) return 'smoke';
  if (cache.status === 'ready') return 'ready';
  if (cache.status === 'failed') return 'failed';
  if (cache.bootScore != null) return 'analyzed';
  return cache.status || 'imported';
}

function renderWizardBar() {
  const wrap = $('#wizardSteps');
  if (!wrap) return;
  wrap.innerHTML = '';
  WIZARD_STEPS.forEach((s, i) => {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'wizard-step';
    if (state.wizard.completed.has(s.id)) btn.classList.add('done');
    if (state.wizard.step === s.id) btn.classList.add('active');
    else if (!canReachStep(s.id)) btn.classList.add('locked');
    btn.dataset.step = s.id;
    btn.innerHTML = `<span class="wizard-num">${state.wizard.completed.has(s.id) ? '✓' : i + 1}</span><span class="wizard-label">${s.label}</span>`;
    btn.addEventListener('click', () => {
      if (canReachStep(s.id)) setWizardStep(s.id);
    });
    wrap.appendChild(btn);
  });

  const idx = WIZARD_STEPS.findIndex((s) => s.id === state.wizard.step);
  const cur = WIZARD_STEPS[idx] || WIZARD_STEPS[0];
  $('#wizardSubtitle').textContent = `Step ${idx + 1} of ${WIZARD_STEPS.length} — ${cur.hint}`;
}

function canReachStep(stepId) {
  const idx = WIZARD_STEPS.findIndex((s) => s.id === stepId);
  if (idx <= 0) return true;
  for (let i = 0; i < idx; i++) {
    if (!state.wizard.completed.has(WIZARD_STEPS[i].id)) return false;
  }
  return true;
}

function markWizardComplete(stepId) {
  state.wizard.completed.add(stepId);
  renderWizardBar();
  updateWizardFooter();
}

function setPipelineStep(step) {
  const mapped = step === 'assure' || step === 'cluster' ? 'assure' : step;
  $$('.pipe-step').forEach((btn) => {
    const id = btn.dataset.step;
    const active = id === step || (step === 'cluster' && id === 'cluster') || (step === 'assure' && id === 'assure' && state.fleetMode !== 'cluster');
    btn.classList.toggle('active', active || (step === 'assure' && id === 'cluster' && state.fleetMode === 'cluster'));
    if (id === 'cluster') {
      btn.classList.toggle('active', state.fleetMode === 'cluster' && (step === 'assure' || step === 'cluster'));
    }
    btn.classList.toggle('locked', Boolean(id && id !== 'cluster' && !canReachStep(id === 'assure' ? 'assure' : id)));
  });
}

function setWizardStep(step) {
  state.wizard.step = step;
  $('#panels')?.setAttribute('data-wizard-step', step);
  setPipelineStep(step);
  renderWizardBar();
  updateWizardFooter();
  window.GuestKitConsole?.renderMissionRail?.();

  const scrollMap = {
    ingest: '#panel-ingest',
    assure: '#panel-fleet',
    plan: '#panel-actions',
    launch: '#panel-results',
  };
  const el = document.querySelector(scrollMap[step]);
  if (el) el.scrollIntoView({ behavior: 'smooth', block: 'start' });
}

function updateWizardFooter() {
  const idx = WIZARD_STEPS.findIndex((s) => s.id === state.wizard.step);
  const back = $('#wizardBackBtn');
  const cont = $('#wizardContinueBtn');
  const action = $('#wizardActionBtn');
  const chain = $('#wizardChainBtn');

  // These wizard-footer controls only exist in the disk-upload layout — the
  // KubeVirt cluster-browse view (and other layouts) can render without
  // them, so every access here must be null-guarded.
  if (back) back.disabled = idx <= 0;

  const stepCfg = WIZARD_ACTIONS[state.wizard.step];
  if (stepCfg && state.selectedVm) {
    if (action) {
      action.textContent = stepCfg.label;
      action.classList.remove('hidden');
      action.dataset.action = stepCfg.action;
    }
  } else {
    action?.classList.add('hidden');
  }

  chain?.classList.toggle('hidden', !(state.selectedVm && state.wizard.step === 'assure'));

  if (cont) {
    if (idx >= WIZARD_STEPS.length - 1) {
      cont.textContent = 'Done';
      cont.disabled = state.wizard.completed.has('launch');
    } else {
      cont.textContent = 'Continue';
      cont.disabled = !state.wizard.completed.has(state.wizard.step);
    }
  }
}

function scrollToPanel(step) {
  setWizardStep(step);
}

async function openClusterFleet() {
  setFleetMode('cluster');
  setWizardStep('assure');
  await loadClusterFleet();
  feed('Showing <strong>KubeVirt cluster</strong> VMs — select one for guest info', 'ok');
}

function zeusVmUrl(namespace, name, section) {
  const base = state.uiConfig?.zeus_url
    || `http://${window.location.hostname || '127.0.0.1'}:30050`;
  const q = new URLSearchParams({ vm: `${namespace}/${name}` });
  if (section) q.set('section', section);
  return `${base.replace(/\/$/, '')}/vms?${q.toString()}`;
}

async function loadUiConfig() {
  try {
    const data = await api('/config');
    state.uiConfig = data.data || null;
    const pathEl = $('#serverStoragePath');
    if (pathEl && state.uiConfig?.storage_path) {
      pathEl.textContent = `Primary storage: ${state.uiConfig.storage_path}`;
    }
  } catch {
    state.uiConfig = null;
  }
}

async function loadServerStorageRoots() {
  try {
    const data = await api('/storage/roots');
    state.serverStorage.roots = data.data || [];
    const select = $('#serverStorageRoot');
    if (!select) return;
    select.innerHTML = state.serverStorage.roots.map((r) =>
      `<option value="${r.id}">${escapeHtml(r.label)} (${escapeHtml(r.path)})</option>`,
    ).join('');
    select.value = String(state.serverStorage.rootId);
  } catch (e) {
    toast(`Server storage unavailable: ${e.message}`, 'err');
  }
}

async function browseServerStorage(path = '', rootId = state.serverStorage.rootId) {
  state.serverStorage.path = path;
  state.serverStorage.rootId = rootId;
  const browser = $('#serverStorageBrowser');
  if (browser) browser.innerHTML = '<p class="storage-browser-empty">Loading…</p>';
  try {
    const q = new URLSearchParams({ root: String(rootId) });
    if (path) q.set('path', path);
    const data = await api(`/storage/browse?${q.toString()}`);
    renderServerStorageBrowser(data.data);
  } catch (e) {
    if (browser) browser.innerHTML = `<p class="storage-browser-empty">${escapeHtml(e.message)}</p>`;
  }
}

function renderServerStorageBrowser(result) {
  if (window.state?.serverStorage) window.state.serverStorage.lastResult = result;
  if (window.GuestKitNebula?.renderServerVaultBrowser) {
    window.GuestKitNebula.renderServerVaultBrowser(result);
    return;
  }
  const browser = $('#serverStorageBrowser');
  const crumb = $('#serverStorageBreadcrumb');
  if (!browser || !result) return;

  const parts = result.path ? result.path.split('/') : [];
  let crumbHtml = `<button type="button" class="crumb-link" data-path="" data-root="${result.root_id}">${escapeHtml(result.root_label || 'root')}</button>`;
  parts.forEach((part, i) => {
    const sub = parts.slice(0, i + 1).join('/');
    crumbHtml += ` <span class="crumb-sep">/</span> <button type="button" class="crumb-link" data-path="${escapeHtml(sub)}" data-root="${result.root_id}">${escapeHtml(part)}</button>`;
  });
  crumb.innerHTML = crumbHtml;
  crumb.querySelectorAll('.crumb-link').forEach((btn) => {
    btn.addEventListener('click', () => browseServerStorage(btn.dataset.path || '', Number(btn.dataset.root || 0)));
  });

  if (!result.entries?.length) {
    browser.innerHTML = '<p class="storage-browser-empty">No disk images in this folder — upload one or pick a subdirectory.</p>';
    return;
  }

  browser.innerHTML = result.entries.map((entry) => {
    const isDir = entry.kind === 'directory';
    const size = entry.size_bytes != null ? fmtBytes(entry.size_bytes) : '';
    const badge = entry.registered ? '<span class="storage-badge registered">imported</span>' : '<span class="storage-badge">on server</span>';
    return `
      <div class="storage-row ${isDir ? 'dir' : 'file'}${entry.registered ? ' registered' : ''}" data-kind="${entry.kind}" data-path="${escapeHtml(entry.path)}" data-root="${result.root_id}">
        <div class="storage-row-main">
          <strong>${escapeHtml(entry.name)}</strong>
          <span class="storage-row-meta">${isDir ? 'folder' : `${escapeHtml(entry.format || 'disk')}${size ? ` · ${size}` : ''}${entry.modified ? ` · ${escapeHtml(entry.modified)}` : ''}`}</span>
        </div>
        ${isDir ? '<span class="storage-row-action">Open →</span>' : `<div class="storage-row-actions">${badge}<button type="button" class="btn primary sm storage-import-btn">Import</button></div>`}
      </div>
    `;
  }).join('');

  browser.querySelectorAll('.storage-row.dir').forEach((row) => {
    row.addEventListener('click', () => browseServerStorage(row.dataset.path, Number(row.dataset.root)));
  });
  browser.querySelectorAll('.storage-import-btn').forEach((btn) => {
    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      const row = btn.closest('.storage-row');
      if (row) importServerDisk(row.dataset.path, Number(row.dataset.root));
    });
  });
}

async function importServerDisk(path, rootId = state.serverStorage.rootId) {
  if (!path) return;
  feed(`Registering server disk <strong>${escapeHtml(path)}</strong>…`);
  try {
    const data = await api('/vms/import-from-storage', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ path, root: rootId }),
    });
    const vm = data.data;
    toast(`Imported ${vm.name}`, 'ok');
    feed(`Server disk registered — <strong>${escapeHtml(vm.name)}</strong>`, 'ok');
    markWizardComplete('ingest');
    await loadFleet();
    await browseServerStorage(state.serverStorage.path, state.serverStorage.rootId);
    const imported = state.vms.find((v) => v.id === vm.id);
    if (imported) {
      selectVm(imported);
      setFleetMode('disks');
      setWizardStep('assure');
    }
  } catch (e) {
    toast(e.message, 'err');
  }
}

function setupServerStorage() {
  $('#serverStorageRefresh')?.addEventListener('click', () => {
    browseServerStorage(state.serverStorage.path, state.serverStorage.rootId);
  });
  $('#serverStorageRoot')?.addEventListener('change', (e) => {
    browseServerStorage('', Number(e.target.value));
  });
  $('#serverStorageUploadBtn')?.addEventListener('click', () => {
    $('#serverFileInput')?.click();
  });
  $('#serverFileInput')?.addEventListener('change', (e) => {
    const file = e.target.files?.[0];
    e.target.value = '';
    if (file) uploadFile(file);
  });
}

function clusterFleetQueryString() {
  const q = new URLSearchParams();
  if (state.fleetFilters.namespace) q.set('namespace', state.fleetFilters.namespace);
  if (state.fleetFilters.search) q.set('search', state.fleetFilters.search);
  if (state.fleetFilters.phase) q.set('phase', state.fleetFilters.phase);
  const s = q.toString();
  return s ? `?${s}` : '';
}

function clusterVmKey(vm) {
  return `${vm.namespace}/${vm.name}`;
}

function setFleetMode(mode) {
  state.fleetMode = mode;
  localStorage.setItem('zyvor.fleetMode', mode);
  $$('.fleet-tab').forEach((tab) => tab.classList.toggle('active', tab.dataset.fleet === mode));
  const subtitle = $('#fleetSubtitle');
  if (subtitle) {
    subtitle.textContent = mode === 'cluster'
      ? 'Live KubeVirt VMs from the cluster — select one for guest agent info.'
      : 'Imported disks staged for offline intelligence.';
  }
  updateFleetEmptyState();
  renderFleet();
  updateSelectionPanels();
  updateFleetToolbar();
}

function updateFleetEmptyState() {
  const emptyText = $('#fleetEmptyText');
  const importBtn = $('#fleetEmptyImport');
  const browseBtn = $('#fleetEmptyBrowseCluster');
  const refreshBtn = $('#fleetEmptyRefresh');
  const isCluster = state.fleetMode === 'cluster';
  if (emptyText) {
    emptyText.textContent = isCluster
      ? 'No KubeVirt VMs found in this cluster.'
      : 'No disks yet — upload an image or browse cluster VMs.';
  }
  importBtn?.classList.toggle('hidden', isCluster);
  browseBtn?.classList.toggle('hidden', !isCluster);
  refreshBtn?.classList.toggle('hidden', !isCluster);
  updateFleetToolbar();
}

function updateFleetToolbar() {
  const isCluster = state.fleetMode === 'cluster';
  $('#fleetUploadBtn')?.classList.toggle('hidden', false);
  $('#fleetBrowseClusterBtn')?.classList.toggle('hidden', isCluster);
  $('#fleetRefreshClusterBtn')?.classList.toggle('hidden', !isCluster);
  $('#fleetClusterFilters')?.classList.toggle('hidden', !isCluster);
  $('#fleetClusterSync')?.classList.toggle('hidden', !isCluster);
  $('#fleetVmtoolsCoverage')?.classList.toggle('hidden', !isCluster);
  $('#fleetVmtoolsPolicy')?.classList.toggle('hidden', !isCluster);
  $('#fleetVmtoolsPolicyBadge')?.classList.toggle('hidden', !isCluster);
  $('#fleetPacketwolfBadge')?.classList.toggle('hidden', !isCluster);
  $('#fleetGuestApprovalsBadge')?.classList.toggle('hidden', !isCluster);
  $('#packetwolfSyncBtn')?.classList.toggle('hidden', !isCluster);
  $('#clusterLifecycle')?.classList.toggle('hidden', !isCluster || !state.selectedClusterVm);
  $('#clusterVmtoolsLifecycle')?.classList.toggle('hidden', !isCluster || !state.selectedClusterVm);
  if (isCluster && state.clusterLastSync) {
    const el = $('#fleetClusterSync');
    if (el) el.textContent = `Synced ${state.clusterLastSync.toLocaleTimeString()}`;
  }
  if (isCluster && state.vmtoolsCoverage) {
    const cov = state.vmtoolsCoverage;
    const el = $('#fleetVmtoolsCoverage');
    if (el) {
      const pending = cov.pending ? ` · ${cov.pending} pending` : '';
      const healthWarn = (cov.health_degraded || 0) + (cov.health_unhealthy || 0);
      const healthPart = healthWarn
        ? ` · ${healthWarn} guest issue${healthWarn === 1 ? '' : 's'}`
        : (cov.health_healthy ? ` · ${cov.health_healthy} healthy` : '');
      el.textContent = `VM Tools ${cov.connected}/${cov.total_vms} live · ${cov.missing} missing${pending}${healthPart}`;
      el.classList.toggle('warn', cov.missing > 0 || cov.pending > 0);
    }
  }
  if (isCluster && state.pendingGuestActions?.length) {
    const el = $('#fleetVmtoolsCoverage');
    if (el) {
      el.textContent += ` · ${state.pendingGuestActions.length} action approval(s)`;
      el.classList.add('warn');
    }
  }
  if (isCluster) {
    const approvalsBadge = $('#fleetGuestApprovalsBadge');
    const pendingCount = state.pendingGuestActions?.length || 0;
    if (approvalsBadge) {
      approvalsBadge.textContent = pendingCount
        ? `${pendingCount} pending approval${pendingCount === 1 ? '' : 's'}`
        : 'Approvals';
      approvalsBadge.classList.toggle('warn', pendingCount > 0);
      approvalsBadge.classList.toggle('live', pendingCount > 0);
    }
    const approvalsPanel = $('#fleetGuestApprovals');
    if (approvalsPanel) {
      const auditCount = (state.guestActionAudit || []).length;
      approvalsPanel.classList.toggle(
        'hidden',
        !pendingCount && !auditCount,
      );
    }
  }
  if (isCluster && state.vmtoolsPolicy) {
    const badge = $('#fleetVmtoolsPolicyBadge');
    const autoInstall = state.vmtoolsPolicy.spec?.autoInstall;
    const autoUpgrade = state.vmtoolsPolicy.spec?.autoUpgrade;
    if (badge) {
      if (autoInstall && autoUpgrade) {
        badge.textContent = 'Auto-install + upgrade';
      } else if (autoInstall) {
        badge.textContent = 'Auto-install on';
      } else if (autoUpgrade) {
        badge.textContent = 'Auto-upgrade on';
      } else {
        badge.textContent = 'Policy off';
      }
      badge.classList.toggle('live', Boolean(autoInstall || autoUpgrade));
    }
    const installToggle = $('#vmtoolsAutoInstall');
    if (installToggle) installToggle.checked = Boolean(autoInstall);
    const upgradeToggle = $('#vmtoolsAutoUpgrade');
    if (upgradeToggle) upgradeToggle.checked = Boolean(autoUpgrade);
  }
  if (isCluster && state.packetwolfFleet) {
    const badge = $('#fleetPacketwolfBadge');
    const snap = state.packetwolfFleet;
    const count = snap.member_count ?? snap.members?.length ?? 0;
    const id = snap.snapshot_id || snap.observed_at || '';
    if (badge) {
      badge.textContent = count ? `PW ${count} VMs` : 'PW idle';
      badge.title = id ? `PacketWolf fleet ${id}` : 'PacketWolf fleet correlation';
      badge.classList.toggle('ok', count > 0);
    }
  }
}

function triggerDiskUpload() {
  setWizardStep('ingest');
  scrollToPanel('ingest');
  const input = $('#fileInput');
  if (input) input.click();
}

function clusterVmStatusClass(vm) {
  const phase = String(vm.phase || vm.status || '').toLowerCase();
  if (phase.includes('run')) return 'running';
  if (phase.includes('pend') || phase.includes('provision') || phase.includes('start') || phase.includes('sched')) return 'pending';
  if (phase.includes('fail') || phase.includes('error')) return 'failed';
  if (phase.includes('stop') || phase.includes('halt')) return 'stopped';
  return 'stopped';
}

function updateSelectionPanels() {
  const clusterSelected = Boolean(state.selectedClusterVm);
  const diskSelected = Boolean(state.selectedVm) && !clusterSelected;
  const commandBay = diskSelected && state.inspectionMode === 'offline';
  const root = document.querySelector('.guestkit-shell');
  root?.classList.toggle('command-bay-active', commandBay);
  $('#panel-actions')?.classList.toggle('hidden', commandBay);
  $('#inspectModeBar')?.classList.toggle('hidden', !diskSelected);
  $('#actionDeck')?.classList.toggle('hidden', !diskSelected || state.inspectionMode !== 'offline');
  $('#clusterDeck')?.classList.toggle('hidden', !clusterSelected);
  $('#clusterLifecycle')?.classList.toggle('hidden', !clusterSelected);
  $('#clusterVmtoolsLifecycle')?.classList.toggle('hidden', !clusterSelected);
  const online = state.inspectionMode === 'online';
  $('#agentDeck')?.classList.toggle('hidden', !diskSelected || !online);
  $('#agentProxyRow')?.classList.toggle('hidden', !online || clusterSelected);
  if (clusterSelected) {
    $$('#clusterDeck [data-cluster-action]').forEach((b) => { b.disabled = false; });
    updateClusterLifecycleButtons(state.selectedClusterVm);
    updateVmtoolsLifecycleButtons(state.selectedClusterVm, state.lastClusterGuestInfo);
    renderClusterDetailDrawer(state.selectedClusterVm, state.lastClusterGuestInfo, state.lastClusterGuestIntel);
  } else {
    $('#clusterDetailDrawer')?.classList.add('hidden');
  }
}

function renderFleet() {
  if (state.fleetMode === 'cluster') {
    renderClusterFleet();
    return;
  }

  const grid = $('#fleetGrid');
  const empty = $('#fleetEmpty');
  const displayVms = window.GuestKitFeatures?.filterFleetVms?.(state.vms)
    || state.vms.filter(isFleetDisk);
  $('#fleetCount').textContent = `${displayVms.length} image${displayVms.length === 1 ? '' : 's'}`;

  grid.querySelectorAll('.vm-card').forEach((c) => c.remove());

  if (!displayVms.length) {
    empty.classList.remove('hidden');
    updateFleetEmptyState();
    return;
  }
  empty.classList.add('hidden');

  [...displayVms].sort((a, b) => fleetRank(a) - fleetRank(b)).forEach((vm) => {
    const cache = getVmCache(vm.id);
    const status = vmStatusLabel(cache, vm);
    const scoreChip = cache.bootScore != null
      ? `<span class="vm-score">${Math.round(cache.bootScore)}</span>`
      : '';
    const smoke = isSmokeDisk(vm);

    const card = document.createElement('button');
    card.type = 'button';
    card.className = 'disk-card vm-card carbon-glass'
      + (state.selectedVm?.id === vm.id ? ' selected' : '')
      + (smoke ? ' smoke' : '');
    if (window.GuestKitConsole?.renderFleetDiskCard) {
      card.innerHTML = window.GuestKitConsole.renderFleetDiskCard(vm, state.selectedVm?.id === vm.id, cache);
      window.GuestKitConsole.bindDiskCardActions(card, vm);
    } else {
      card.innerHTML = `
      <span class="vm-format">${vm.format || 'disk'}</span>
      <span class="vm-status ${status}">${status}</span>
      ${scoreChip}
      <p class="vm-name">${escapeHtml(vm.name || 'unnamed')}</p>
      <p class="vm-meta">${fmtBytes(vm.size_bytes)} · ${vm.id.slice(0, 8)}…</p>
      ${smoke ? '<p class="vm-hint">Not a bootable VM</p>' : ''}
    `;
    }
    card.addEventListener('click', () => selectVm(vm));
    grid.appendChild(card);
    if (state.selectedVm?.id === vm.id) {
      requestAnimationFrame(() => card.scrollIntoView({ block: 'nearest', behavior: 'smooth' }));
    }
  });
}

function clusterToolsLabel(vm) {
  if (vm.is_windows) return 'virtio-win';
  if (vm.tools_connected === true) return 'tools live';
  const stateLabel = vm.guest_tools || 'missing';
  const target = state.vmtoolsBundle?.version;
  const outdated = vm.tools_version && target && vm.tools_version !== target;
  if (stateLabel === 'connected') return outdated ? 'tools outdated' : 'tools live';
  if (stateLabel === 'pending') return 'tools pending';
  return 'no tools';
}

function renderClusterFleet() {
  const grid = $('#fleetGrid');
  const empty = $('#fleetEmpty');
  const vms = state.clusterVms || [];
  $('#fleetCount').textContent = `${vms.length} VM${vms.length === 1 ? '' : 's'}`;

  grid.querySelectorAll('.vm-card, .fleet-ns-header').forEach((c) => c.remove());

  if (!vms.length) {
    empty.classList.remove('hidden');
    updateFleetEmptyState();
    return;
  }
  empty.classList.add('hidden');

  const namespaces = [...new Set(vms.map((v) => v.namespace))].sort();
  const groupByNs = namespaces.length > 1;

  namespaces.forEach((ns) => {
    const group = vms.filter((v) => v.namespace === ns);
    if (groupByNs) {
      const header = document.createElement('div');
      header.className = 'fleet-ns-header';
      header.textContent = ns;
      grid.appendChild(header);
    }
    group.forEach((vm) => {
      const key = clusterVmKey(vm);
      const selected = state.selectedClusterVm && clusterVmKey(state.selectedClusterVm) === key;
      const agent = vm.guest_agent_connected;
      const agentLabel = agent === true ? 'agent on' : agent === false ? 'no agent' : 'unknown';
      const osHint = vm.is_windows ? 'windows' : (vm.os_name ? vm.os_name.split(/\s+/)[0].toLowerCase() : '');
      const toolsLabel = clusterToolsLabel(vm);
      const toolsClass = toolsLabel.includes('live') ? 'live' : toolsLabel.includes('pending') ? 'warn' : 'muted';
      const healthLabel = vm.guest_health && vm.guest_health !== 'healthy'
        ? vm.guest_health
        : (vm.health_score != null && vm.health_score < 80 ? 'degraded' : null);
      const healthClass = healthLabel === 'unhealthy' ? 'err' : healthLabel ? 'warn' : '';
      const card = document.createElement('button');
      card.type = 'button';
      card.className = 'vm-card cluster carbon-glass' + (selected ? ' selected' : '');
      card.innerHTML = `
        <span class="vm-format">kubevirt</span>
        <span class="vm-status ${clusterVmStatusClass(vm)}">${escapeHtml(vm.status || vm.phase || 'Unknown')}</span>
        <span class="vm-tools-chip ${toolsClass}">${escapeHtml(toolsLabel)}</span>
        ${healthLabel ? `<span class="vm-tools-chip ${healthClass}">${escapeHtml(healthLabel)}${vm.health_score != null ? ` ${vm.health_score}` : ''}</span>` : ''}
        ${vm.packetwolf_correlation ? `<span class="vm-tools-chip ok" title="PacketWolf correlated">PW</span>` : ''}
        <p class="vm-name">${escapeHtml(vm.namespace)}/${escapeHtml(vm.name)}</p>
        <p class="vm-meta">${escapeHtml(vm.ip_address || 'no IP')} · ${escapeHtml(vm.node || 'unscheduled')} · ${agentLabel}${osHint ? ` · ${escapeHtml(osHint)}` : ''}${vm.tools_version ? ` · v${escapeHtml(vm.tools_version)}` : ''}</p>
      `;
      card.addEventListener('click', () => selectClusterVm(vm));
      grid.appendChild(card);
    });
  });
}

function renderClusterDetailDrawer(vm, info, intel) {
  const drawer = $('#clusterDetailDrawer');
  if (!drawer || !vm) {
    drawer?.classList.add('hidden');
    return;
  }
  drawer.classList.remove('hidden');
  $('#clusterDrawerTitle').textContent = `${vm.namespace}/${vm.name}`;
  const rows = [
    ['Namespace', vm.namespace],
    ['Status', vm.status || vm.phase || 'Unknown'],
    ['Node', vm.node || '—'],
    ['Age', vm.age || '—'],
    ['Root PVC', vm.root_pvc || '—'],
    ['IP', vm.ip_address || '—'],
    ['OS', info?.os_name ? `${info.os_name} ${info.os_version || ''}`.trim() : (vm.os_name || '—')],
    ['Agent', info?.guest_agent_version ? `v${info.guest_agent_version}` : (info?.agent_connected ? 'connected' : '—')],
  ];
  const meta = $('#clusterDrawerMeta');
  if (meta) {
    meta.innerHTML = rows.map(([label, value]) => `<dt>${escapeHtml(label)}</dt><dd>${escapeHtml(String(value))}</dd>`).join('');
  }
  const links = $('#clusterDrawerLinks');
  if (links) {
    links.innerHTML = `
      <a class="btn glass sm" href="${zeusVmUrl(vm.namespace, vm.name)}" target="_blank" rel="noopener noreferrer">Zeus console</a>
      <a class="btn glass sm" href="${zeusVmUrl(vm.namespace, vm.name, 'guest')}" target="_blank" rel="noopener noreferrer">Guest tools</a>
    `;
  }
  if (info?.interfaces?.length) {
    const ifaceRows = info.interfaces.map((i) => {
      const name = i.name || 'iface';
      const ip = i.ipAddress || '—';
      const mac = i.mac || '';
      return `<dt>${escapeHtml(name)}</dt><dd>${escapeHtml(ip)}${mac ? ` · ${escapeHtml(mac)}` : ''}</dd>`;
    }).join('');
    meta.innerHTML += `<dt>Interfaces</dt>${ifaceRows}`;
  }
  const drawerLinks = $('#clusterDrawerLinks');
  if (drawerLinks && intel?.guest_health) {
    const healthEl = document.createElement('div');
    healthEl.className = 'guest-intel-drawer';
    renderGuestIntelligenceCard(healthEl, intel);
    drawerLinks.appendChild(healthEl);
  }
}

function updateClusterLifecycleButtons(vm) {
  if (!vm) return;
  const phase = String(vm.phase || vm.status || '').toLowerCase();
  const running = phase.includes('run');
  const stopped = phase.includes('stop') || phase.includes('halt') || !vm.phase;
  const startBtn = $('#clusterStartBtn');
  const stopBtn = $('#clusterStopBtn');
  const restartBtn = $('#clusterRestartBtn');
  if (startBtn) startBtn.disabled = running;
  if (stopBtn) stopBtn.disabled = !running;
  if (restartBtn) restartBtn.disabled = !running;
  if (startBtn && stopped && !running) startBtn.disabled = false;
  if (state.selectedClusterVm?.name === vm.name && state.selectedClusterVm?.namespace === vm.namespace) {
    setClusterDockEnabled(true);
  }
}

function updateVmtoolsLifecycleButtons(vm, info) {
  if (!vm) return;
  const phase = String(vm.phase || vm.status || '').toLowerCase();
  const running = phase.includes('run');
  const agentOk = Boolean(info?.agent_connected);
  const isWin = Boolean(info?.is_windows || vm.is_windows);
  const quiesceBtn = $('#clusterQuiesceBtn');
  const unquiesceBtn = $('#clusterUnquiesceBtn');
  const rebootBtn = $('#clusterGuestRebootBtn');
  const shutdownBtn = $('#clusterGuestShutdownBtn');
  const toolsEnabled = running && agentOk && !isWin;
  if (quiesceBtn) quiesceBtn.disabled = !toolsEnabled;
  if (unquiesceBtn) unquiesceBtn.disabled = !toolsEnabled;
  if (rebootBtn) rebootBtn.disabled = !toolsEnabled;
  if (shutdownBtn) shutdownBtn.disabled = !running;
}

function selectClusterVm(vm) {
  state.selectedClusterVm = vm;
  state.selectedVm = null;
  renderFleet();
  setInspectionMode('online');
  $('#selectedVmTitle').textContent = `${vm.namespace}/${vm.name}`;
  const meta = $('#selectedVmMeta');
  meta.classList.remove('vm-warn');
  meta.textContent = `${vm.status || vm.phase || 'Unknown'} · ${vm.ip_address || 'no guest IP'} · ${vm.node || 'no node'}`;
  $$('.action-card[data-action]').forEach((b) => { b.disabled = true; });
  setClusterDockEnabled(true);
  updateSelectionPanels();
  updateCopilotPlaceholder();
  markWizardComplete('ingest');
  if (state.wizard.step === 'ingest') setWizardStep('assure');
  feed(`Selected cluster VM <strong>${escapeHtml(vm.namespace)}/${escapeHtml(vm.name)}</strong>`, 'ok');
  renderClusterDetailDrawer(vm, null, null);
  updateClusterLifecycleButtons(vm);
  fetchClusterGuestInfo(false);
  window.GuestKitConsole?.onSelectVmConsole?.();
}

async function loadClusterNamespaces() {
  try {
    const data = await api('/kubevirt/namespaces');
    state.clusterNamespaces = data.data || [];
    const select = $('#fleetNamespaceFilter');
    if (!select) return;
    const current = state.fleetFilters.namespace;
    select.innerHTML = '<option value="">All namespaces</option>'
      + state.clusterNamespaces.map((ns) => `<option value="${escapeHtml(ns)}"${ns === current ? ' selected' : ''}>${escapeHtml(ns)}</option>`).join('');
  } catch {
    /* optional */
  }
}

async function loadPendingGuestActions() {
  try {
    const data = await api('/guest-actions/pending');
    state.pendingGuestActions = data.data || [];
    updateFleetToolbar();
    renderGuestApprovalsPanel();
  } catch {
    state.pendingGuestActions = [];
    renderGuestApprovalsPanel();
  }
}

async function loadGuestActionAudit() {
  try {
    const data = await api('/guest-actions/audit?limit=40');
    state.guestActionAudit = data.data || [];
    renderGuestApprovalsPanel();
  } catch {
    state.guestActionAudit = [];
    renderGuestApprovalsPanel();
  }
}

async function approveGuestAction(actionId) {
  feed(`Approving guest action <strong>${escapeHtml(actionId)}</strong>…`);
  try {
    await api(`/guest-actions/${encodeURIComponent(actionId)}/approve`, { method: 'POST' });
    toast('Guest action approved', 'ok');
    await loadPendingGuestActions();
    await loadGuestActionAudit();
    await fetchClusterGuestInfo(false);
  } catch (e) {
    toast(e.message, 'err');
  }
}

async function rejectGuestAction(actionId) {
  feed(`Rejecting guest action <strong>${escapeHtml(actionId)}</strong>…`);
  try {
    await api(`/guest-actions/${encodeURIComponent(actionId)}/reject`, { method: 'POST' });
    toast('Guest action rejected', 'ok');
    await loadPendingGuestActions();
    await loadGuestActionAudit();
    await fetchClusterGuestInfo(false);
  } catch (e) {
    toast(e.message, 'err');
  }
}

function renderGuestApprovalsPanel() {
  const panel = $('#fleetGuestApprovals');
  if (!panel) return;
  const pending = state.pendingGuestActions || [];
  const audit = (state.guestActionAudit || []).filter((a) => a.status !== 'pending');
  const pendingEl = $('#fleetGuestApprovalsPending');
  const auditEl = $('#fleetGuestApprovalsAudit');
  if (pendingEl) {
    if (!pending.length) {
      pendingEl.innerHTML = '<p class="muted">No pending approvals.</p>';
    } else {
      pendingEl.innerHTML = '<ul class="guest-intel-recs">' + pending.map((a) => {
        const who = a.requested_by ? ` <span class="muted">by ${escapeHtml(a.requested_by)}</span>` : '';
        const vm = `${escapeHtml(a.namespace)}/${escapeHtml(a.vm_name)}`;
        const unit = a.unit ? ` ${escapeHtml(a.unit)}` : '';
        return `<li><strong>${vm}</strong> ${escapeHtml(a.action)}${unit}${who}
          <button type="button" class="btn glass sm accent" data-guest-approve="${escapeHtml(a.id)}">Approve</button>
          <button type="button" class="btn glass sm" data-guest-reject="${escapeHtml(a.id)}">Reject</button></li>`;
      }).join('') + '</ul>';
    }
  }
  if (auditEl) {
    if (!audit.length) {
      auditEl.innerHTML = '<p class="muted">No recent approval history.</p>';
    } else {
      auditEl.innerHTML = '<ul class="guest-intel-recs guest-action-audit">' + audit.slice(0, 20).map((a) => {
        const vm = `${escapeHtml(a.namespace)}/${escapeHtml(a.vm_name)}`;
        const unit = a.unit ? ` ${escapeHtml(a.unit)}` : '';
        const actor = a.approved_by || a.rejected_by || '';
        const when = a.approved_at || a.rejected_at || a.created_at || '';
        const statusClass = a.status === 'approved' ? 'ok' : 'warn';
        return `<li class="${statusClass}"><code>${escapeHtml(when)}</code> <strong>${vm}</strong> ${escapeHtml(a.action)}${unit}
          <span class="muted">${escapeHtml(a.status)}${actor ? ` by ${escapeHtml(actor)}` : ''}</span></li>`;
      }).join('') + '</ul>';
    }
  }
}

async function loadVmtoolsCoverage() {
  try {
    const data = await api('/vmtools/coverage');
    state.vmtoolsCoverage = data.data;
    updateFleetToolbar();
  } catch {
    /* optional */
  }
}

async function loadVmtoolsBundle() {
  try {
    const data = await api('/vmtools/bundle');
    state.vmtoolsBundle = data.data;
  } catch {
    /* optional */
  }
}

async function loadPacketwolfFleet() {
  try {
    const data = await api('/packetwolf/fleet-snapshot');
    state.packetwolfFleet = data.data || {};
    updateFleetToolbar();
  } catch {
    /* optional when PacketWolf not configured */
  }
}

async function syncPacketwolfFleet() {
  feed('Running PacketWolf fleet correlation…');
  try {
    const data = await api('/packetwolf/fleet-correlate', { method: 'POST' });
    state.packetwolfFleet = data.data || {};
    updateFleetToolbar();
    const count = state.packetwolfFleet.member_count ?? state.packetwolfFleet.members?.length ?? 0;
    toast(`PacketWolf fleet sync: ${count} VM(s)`, 'ok');
    await loadClusterFleet();
  } catch (e) {
    toast(e.message, 'err');
  }
}

async function loadVmtoolsPolicy() {
  try {
    const data = await api('/vmtools/policy');
    state.vmtoolsPolicy = data.data;
    updateFleetToolbar();
  } catch {
    /* optional */
  }
}

async function saveVmtoolsPolicy() {
  const autoInstall = Boolean($('#vmtoolsAutoInstall')?.checked);
  const autoUpgrade = Boolean($('#vmtoolsAutoUpgrade')?.checked);
  feed(autoInstall || autoUpgrade
    ? 'Updating VM Tools fleet policy…'
    : 'Disabling VM Tools fleet policy…');
  try {
    const data = await api('/vmtools/policy', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        spec: {
          selector: {},
          autoInstall,
          autoUpgrade,
          channel: 'stable',
          rebootPolicy: 'if-needed',
          maxConcurrent: 3,
        },
      }),
    });
    state.vmtoolsPolicy = data.data;
    updateFleetToolbar();
    toast('VM Tools policy saved', 'ok');
  } catch (e) {
    toast(e.message, 'err');
  }
}

async function reconcileVmtoolsFleet() {
  feed('Reconciling VM Tools policy across cluster fleet…');
  try {
    const data = await api('/vmtools/policy/reconcile', { method: 'POST' });
    const r = data.data;
    const parts = [
      `scanned ${r.scanned}`,
      `matched ${r.matched}`,
      r.installed ? `installed ${r.installed}` : null,
      r.pending ? `pending ${r.pending}` : null,
      r.upgraded ? `upgraded ${r.upgraded}` : null,
      `skipped ${r.skipped}`,
    ].filter(Boolean);
    const summary = parts.join(', ');
    const ok = (r.installed + r.pending + r.upgraded) > 0 && !(r.errors?.length);
    toast(summary, ok ? 'ok' : 'err');
    feed(`${summary}${r.errors?.length ? ` — ${r.errors.join('; ')}` : ''}`);
    await loadClusterFleet();
    await loadVmtoolsCoverage();
    await loadVmtoolsPolicy();
  } catch (e) {
    toast(e.message, 'err');
  }
}

async function loadClusterFleet() {
  if (state.clusterFleetLoading) return;
  state.clusterFleetLoading = true;
  try {
    await loadClusterNamespaces();
    const data = await api(`/kubevirt/vms${clusterFleetQueryString()}`);
    state.clusterVms = data.data || [];
    state.clusterLastSync = new Date();
    if (state.selectedClusterVm) {
      const key = clusterVmKey(state.selectedClusterVm);
      const fresh = state.clusterVms.find((v) => clusterVmKey(v) === key);
      state.selectedClusterVm = fresh || null;
      if (fresh) updateClusterLifecycleButtons(fresh);
    }
    renderFleet();
    updateFleetToolbar();
    await loadVmtoolsCoverage();
    await loadVmtoolsPolicy();
    await loadVmtoolsBundle();
    await loadPacketwolfFleet();
    await loadPendingGuestActions();
    await loadGuestActionAudit();
    if (!state.clusterVms.length && !state.fleetFilters.search && !state.fleetFilters.namespace && !state.fleetFilters.phase) {
      toast('No KubeVirt VMs found — create VMs in Zeus OS first', 'err');
    }
  } catch (e) {
    toast(`Cluster VM load failed: ${e.message}`, 'err');
  } finally {
    state.clusterFleetLoading = false;
  }
}

async function fetchClusterGuestInfo(showToast = true) {
  const vm = state.selectedClusterVm;
  if (!vm) {
    toast('Select a cluster VM first', 'err');
    return;
  }
  feed(`Fetching guest info for <strong>${escapeHtml(vm.namespace)}/${escapeHtml(vm.name)}</strong>…`);
  setActiveTab('summary');
  try {
    const [guestRes, intelRes, bootRes, statusRes, doctorRes] = await Promise.allSettled([
      api(`/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}/guest-agent`),
      api(`/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}/guest/info`),
      api(`/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}/boot-inspect`),
      api(`/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}/guest/status`),
      api(`/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}/guest/doctor`),
    ]);
    if (guestRes.status === 'rejected') throw guestRes.reason;
    const info = guestRes.value.data;
    state.lastClusterGuestInfo = info;
    state.lastClusterGuestIntel = intelRes.status === 'fulfilled'
      ? (guestEnvelope(intelRes.value).data ?? intelRes.value.data)
      : null;
    state.lastClusterBootInspect = bootRes.status === 'fulfilled' ? bootRes.value.data : null;
    state.lastGuestControlStatus = statusRes.status === 'fulfilled' ? guestEnvelope(statusRes.value) : null;
    state.lastGuestDoctor = doctorRes.status === 'fulfilled' ? guestEnvelope(doctorRes.value) : null;
    renderClusterGuestSummary(info, state.lastClusterBootInspect);
    const control = state.lastGuestControlStatus;
    if (control?.controlState) {
      setAgentControlStatus(control);
    } else {
      setAgentStatus(info.agent_connected ? 'agent connected' : info.health, info.agent_connected);
    }
    updateExecWarningBanner(control);
    if (showToast) toast(info.agent_connected ? 'Guest agent connected' : 'Guest agent not connected', info.agent_connected ? 'ok' : 'err');
    feed(info.message);
    if (state.lastClusterBootInspect?.message) feed(state.lastClusterBootInspect.message);
  } catch (e) {
    toast(e.message, 'err');
  }
}

async function fetchClusterBootInspect(showToast = true) {
  const vm = state.selectedClusterVm;
  if (!vm) {
    toast('Select a cluster VM first', 'err');
    return;
  }
  feed(`Boot inspect for <strong>${escapeHtml(vm.namespace)}/${escapeHtml(vm.name)}</strong>…`);
  try {
    const data = await api(
      `/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}/boot-inspect`,
      { method: 'POST' },
    );
    state.lastClusterBootInspect = data.data;
    if (state.lastClusterGuestInfo) {
      renderClusterGuestSummary(state.lastClusterGuestInfo, state.lastClusterBootInspect);
    } else {
      showRaw({ mode: 'kubevirt-boot-inspect', boot_inspect: state.lastClusterBootInspect });
    }
    if (showToast) {
      toast(
        state.lastClusterBootInspect.available ? 'Boot inspect complete' : 'Boot hints from VM spec',
        state.lastClusterBootInspect.available ? 'ok' : 'err',
      );
    }
    feed(state.lastClusterBootInspect.message || 'Boot inspect finished');
  } catch (e) {
    toast(e.message, 'err');
  }
}

function buildClusterCopilotBriefing(info, vm, bootInspect) {
  const agentOk = info.agent_connected;
  const running = info.vmi_running;
  const isWin = info.is_windows;
  const os = info.os_name
    ? `${info.os_name} ${info.os_version || ''}`.trim()
    : (bootInspect?.os_release || (isWin ? 'Windows' : 'Unknown OS'));
  const ips = (info.interfaces || []).map((i) => i.ipAddress).filter(Boolean).join(', ') || vm.ip_address || 'no IP';
  const recommendedActions = [];
  const evidenceHighlights = [];
  let readiness;
  let headline;
  let summary;
  let bootScore;
  let blockerCount = 0;
  let nextWorkflow = 'cluster-guest-info';

  if (!running) {
    readiness = 'blocked';
    headline = 'VM is not running';
    summary = 'Start the VM in Zeus OS before live guest agent inspection.';
    bootScore = 0;
    blockerCount = 1;
    nextWorkflow = 'cluster-start';
    recommendedActions.push({
      priority: 1,
      title: 'Start VM',
      detail: 'Boot the VM, then refresh guest info.',
      workflow: 'cluster-start',
    });
  } else if (!agentOk) {
    readiness = isWin ? 'caution' : 'blocked';
    headline = isWin ? 'Guest agent missing — install virtio-win' : 'Guest agent not connected';
    summary = info.message;
    bootScore = isWin ? 45 : 30;
    blockerCount = 1;
    nextWorkflow = isWin ? 'cluster-zeus' : 'cluster-install-agent';
    recommendedActions.push({
      priority: 1,
      title: isWin ? 'Install QEMU Guest Agent' : 'Install GuestKit agent',
      detail: isWin
        ? 'Open Zeus OS Guest Tools and attach virtio-win.iso.'
        : 'Merge GuestKit cloud-init and restart the VM.',
      workflow: isWin ? 'cluster-zeus' : 'cluster-install-agent',
    });
  } else {
    readiness = 'ready';
    headline = `Live guest healthy — ${os}`;
    summary = `${info.hostname || vm.name} is reachable at ${ips}. Guest agent ${info.guest_agent_version ? `v${info.guest_agent_version}` : 'connected'}. Use offline ingest for full migration plan and YAML.`;
    bootScore = 85;
    nextWorkflow = 'cluster-boot-inspect';
    recommendedActions.push({
      priority: 1,
      title: 'Run boot inspect',
      detail: 'Collect offline boot hints from the root PVC when the VM is stopped.',
      workflow: 'cluster-boot-inspect',
    });
    recommendedActions.push({
      priority: 2,
      title: 'Export root disk',
      detail: 'Copy cluster root PVC into Zyvor ingest for Doctor, Migrate, and YAML.',
      workflow: 'cluster-export-disk',
    });
  }

  if (info.hostname) {
    evidenceHighlights.push({ ref: 'guest.hostname', label: 'Hostname', detail: info.hostname });
  }
  if (os) {
    evidenceHighlights.push({ ref: 'guest.os', label: 'Operating system', detail: os });
  }
  if (ips) {
    evidenceHighlights.push({ ref: 'guest.network', label: 'Network', detail: ips });
  }
  if (info.guest_agent_version) {
    evidenceHighlights.push({ ref: 'guest.agent', label: 'Guest agent', detail: `v${info.guest_agent_version}` });
  }
  if (bootInspect) {
    evidenceHighlights.push({
      ref: 'boot.inspect',
      label: 'Boot inspect',
      detail: [bootInspect.os_release, bootInspect.bootloader, bootInspect.cloud_init_present ? 'cloud-init' : null]
        .filter(Boolean)
        .join(' · ') || bootInspect.message,
    });
    if (bootInspect.available && bootScore < 90) bootScore = 90;
    if (bootInspect.source === 'vm_spec_heuristic' && !bootInspect.available) {
      recommendedActions.unshift({
        priority: 1,
        title: 'Stop VM for boot inspect',
        detail: 'Stop the VM, then re-run boot inspect on the root disk.',
        workflow: 'cluster-stop-boot-inspect',
      });
    }
  }
  if (vm.root_pvc) {
    evidenceHighlights.push({ ref: 'cluster.root_pvc', label: 'Root PVC', detail: vm.root_pvc });
  }

  const insights = [
    {
      id: 'agent_status',
      question: 'Is the guest agent connected?',
      answer: agentOk
        ? `Yes — ${info.health}. ${info.message}`
        : `No — ${info.message}. ${isWin ? 'Use Zeus OS Guest Tools (virtio-win).' : 'Use Install agent to merge GuestKit cloud-init.'}`,
    },
    {
      id: 'blockers',
      question: 'What is blocking live inspection?',
      answer: !running
        ? 'VM is not in Running phase.'
        : !agentOk
          ? (isWin ? 'Windows needs QEMU Guest Agent from virtio-win ISO.' : 'Linux needs qemu-guest-agent or GuestKit agent.')
          : 'No blockers — live inspection is available.',
    },
    {
      id: 'fix_first',
      question: 'What should I fix first?',
      answer: recommendedActions[0]?.detail || 'Refresh guest info.',
    },
    {
      id: 'ready',
      question: 'Is this VM ready for migration tooling?',
      answer: agentOk && running
        ? 'Live agent is connected — import the root disk for full boot score, driver plan, and KubeVirt YAML export.'
        : 'Resolve guest agent connectivity first, then import the disk for offline Doctor and Migrate workflows.',
    },
    {
      id: 'evidence',
      question: 'What do we know about this guest?',
      answer: [
        os,
        info.hostname && `hostname ${info.hostname}`,
        ips && `IP ${ips}`,
        info.guest_agent_version && `agent ${info.guest_agent_version}`,
        bootInspect?.bootloader && `${bootInspect.bootloader} bootloader`,
      ].filter(Boolean).join('. '),
    },
    {
      id: 'migration_changes',
      question: 'What migration steps apply to a live KubeVirt VM?',
      answer: 'Live cluster VMs use the guest agent for assurance. For boot score, driver injection, and YAML export, attach the root disk to Zyvor ingest and run offline Doctor → Migrate → Provision.',
    },
  ];

  return {
    readiness,
    headline,
    summary,
    boot_score: bootScore,
    migration_score: null,
    blocker_count: blockerCount,
    warning_count: agentOk ? 0 : 1,
    evidence_digest: {
      os,
      architecture: '',
      bootloader: bootInspect?.bootloader || (running ? 'live' : 'unknown'),
      root_filesystem: '',
      kernel_count: 0,
      fstab_entries: bootInspect?.fstab_valid === false ? 0 : 1,
      virtio_modules_loaded: agentOk,
      vm_tools: agentOk ? ['qemu-guest-agent'] : [],
      selinux: '',
    },
    evidence_highlights: evidenceHighlights,
    recommended_actions: recommendedActions,
    insights,
    next_workflow: nextWorkflow,
  };
}

async function fetchClusterCopilotBriefing(info, vm, bootInspect) {
  try {
    const data = await api(
      `/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}/copilot/briefing`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          guest_agent: info,
          boot_inspect: bootInspect || null,
        }),
      },
    );
    return data.data;
  } catch {
    return buildClusterCopilotBriefing(info, vm, bootInspect);
  }
}

async function renderClusterCopilot(info, vm, bootInspect) {
  const briefing = await fetchClusterCopilotBriefing(info, vm, bootInspect);
  state.lastClusterBriefing = briefing;
  renderCopilot(briefing, { mode: 'kubevirt-live', guest_agent: info, boot_inspect: bootInspect });
  if (briefing.boot_score != null) setScore(briefing.boot_score);
  markWizardComplete('assure');
  updateWizardFooter();
  setActiveTab('copilot');
  feed('Cluster <strong>Ask Zeus</strong> briefing ready', 'ok');
}

function updateCopilotPlaceholder() {
  const ph = $('#copilotPlaceholder');
  if (!ph) return;
  ph.textContent = state.selectedClusterVm
    ? 'Fetch guest info on a cluster VM — Copilot briefs live agent status automatically.'
    : 'Run Doctor with explain to unlock Ask Zeus.';
}

function runWorkflow(workflow) {
  if (!workflow) return;
  if (workflow.startsWith('cluster-')) {
    const vm = state.selectedClusterVm;
    if (!vm) {
      toast('Select a cluster VM first', 'err');
      return;
    }
    if (workflow === 'cluster-install-agent') installClusterGuestAgent();
    else if (workflow === 'cluster-guest-info') fetchClusterGuestInfo(true);
    else if (workflow === 'cluster-boot-inspect') fetchClusterBootInspect(true);
    else if (workflow === 'cluster-stop-boot-inspect') stopThenBootInspect();
    else if (workflow === 'cluster-start') clusterLifecycle('start');
    else if (workflow === 'cluster-stop') clusterLifecycle('stop');
    else if (workflow === 'cluster-restart') clusterLifecycle('restart');
    else if (workflow === 'cluster-export-disk') exportClusterDisk(false);
    else if (workflow === 'cluster-apply-yaml') applyYamlToCluster();
    else if (workflow === 'cluster-zeus') {
      window.open(zeusVmUrl(vm.namespace, vm.name, 'guest'), '_blank', 'noopener,noreferrer');
    }
    return;
  }
  runAction(workflow);
}

function renderGuestIntelligenceCard(container, intel) {
  if (!intel) return;
  const health = intel.guest_health;
  if (!health) return;
  const level = health.guest_health || 'unknown';
  const levelClass = level === 'healthy' ? 'ok' : level === 'degraded' ? 'warn' : 'err';
  let html = `<div class="guest-intel-card glass-inset"><h3>Guest Intelligence</h3>`;
  html += `<p class="finding ${levelClass}"><strong>Health</strong> ${escapeHtml(level)}`;
  if (health.score) html += ` · score ${health.score}`;
  html += ` · systemd ${escapeHtml(health.systemd_state || '—')} · ${health.failed_units || 0} failed unit(s)</p>`;

  if (health.components) {
    html += '<div class="guest-intel-chips">';
    const chipLabels = {
      boot: 'Boot',
      systemd: 'Services',
      network: 'Network',
      dns: 'DNS',
      storage: 'Storage',
      security: 'Security',
      agent: 'Agent',
    };
    Object.entries(chipLabels).forEach(([key, label]) => {
      const compLevel = health.components[key] || 'unknown';
      const chipClass = compLevel === 'healthy' ? 'ok' : compLevel === 'degraded' ? 'warn' : compLevel === 'unhealthy' ? 'err' : '';
      html += `<span class="guest-intel-chip ${chipClass}">${escapeHtml(label)}: ${escapeHtml(compLevel)}</span>`;
    });
    html += '</div>';
  }

  if (health.reasons?.length) {
    html += `<p class="finding warn"><strong>Reasons</strong> ${escapeHtml(health.reasons.join(', '))}</p>`;
  }

  if (health.journal_hints?.length) {
    html += '<p class="finding warn"><strong>Journal hints</strong></p><ul class="guest-intel-recs">';
    health.journal_hints.slice(0, 5).forEach((hint) => {
      html += `<li>${escapeHtml(hint)}</li>`;
    });
    html += '</ul>';
  }

  if (health.network) {
    const dnsOk = health.network.dns_working;
    html += `<p class="finding ${dnsOk ? 'ok' : 'warn'}"><strong>Network</strong> DNS ${dnsOk ? 'ok' : 'broken'} · default route ${health.network.default_route ? 'yes' : 'no'}</p>`;
  }
  if (health.storage) {
    html += `<p class="finding ${health.storage.root_usage_percent >= 90 ? 'warn' : 'ok'}"><strong>Storage</strong> root ${health.storage.root_usage_percent || 0}% · inodes ${health.storage.inode_usage_percent || 0}%</p>`;
  }

  const bootAnalysis = intel.boot_analysis;
  if (bootAnalysis?.slow_units?.length) {
    html += '<p class="finding ok"><strong>Slow boot units</strong></p><ul class="guest-intel-recs">';
    bootAnalysis.slow_units.slice(0, 5).forEach((u) => {
      html += `<li>${escapeHtml(u.name)} — ${u.time_ms}ms</li>`;
    });
    html += '</ul>';
    if (bootAnalysis.total_boot_time_ms) {
      html += `<p class="finding ok">Total boot ~${Math.round(bootAnalysis.total_boot_time_ms / 1000)}s (kernel ${bootAnalysis.kernel_time_ms || 0}ms · userspace ${bootAnalysis.userspace_time_ms || 0}ms)</p>`;
    }
  }

  if (health.critical_services?.length) {
    html += '<ul class="guest-intel-services">';
    health.critical_services.slice(0, 5).forEach((svc) => {
      const failure = svc.last_failure ? `<br><em>${escapeHtml(svc.last_failure)}</em>` : '';
      html += `<li class="finding err"><strong>${escapeHtml(svc.name)}</strong> — ${escapeHtml(svc.reason || svc.state)}${failure}<br><span>${escapeHtml(svc.suggested_action || '')}</span></li>`;
    });
    html += '</ul>';
  }
  if (health.recommendations?.length) {
    html += '<p class="finding ok"><strong>Recommendations</strong></p><ul class="guest-intel-recs">';
    health.recommendations.slice(0, 3).forEach((r) => {
      html += `<li>${escapeHtml(r.title)} — ${escapeHtml(r.detail)}</li>`;
    });
    html += '</ul>';
  }
  if (intel.report_source) {
    html += `<p class="finding ok"><strong>Report source</strong> ${escapeHtml(intel.report_source)}</p>`;
  }

  if (intel.packetwolf) {
    const pw = intel.packetwolf;
    const parts = [];
    if (pw.correlation) parts.push(`VM ${escapeHtml(pw.correlation)}`);
    if (pw.correlation_at) parts.push(`at ${escapeHtml(pw.correlation_at)}`);
    if (pw.fleet_correlation) parts.push(`fleet ${escapeHtml(pw.fleet_correlation)}`);
    if (pw.fleet_count) parts.push(`${escapeHtml(pw.fleet_count)} VMs`);
    if (parts.length) {
      html += `<p class="finding ok"><strong>PacketWolf</strong> ${parts.join(' · ')}</p>`;
    }
  }

  if (intel.metrics?.ips?.length) {
    html += `<p class="finding ok"><strong>Guest IPs</strong> ${escapeHtml(intel.metrics.ips.join(', '))}</p>`;
  }

  if (intel.recent_events?.length) {
    html += '<p class="finding ok"><strong>Recent systemd events</strong></p><ul class="guest-intel-recs">';
    intel.recent_events.slice(0, 8).forEach((ev) => {
      const detail = ev.detail || ev.kind || '';
      html += `<li><code>${escapeHtml(ev.timestamp || '')}</code> ${escapeHtml(ev.unit || '')} — ${escapeHtml(detail)}</li>`;
    });
    html += '</ul>';
  }

  html += '<div class="guest-intel-actions">';
  html += `<button type="button" class="btn glass sm" data-guest-action="collect-bundle">Collect support bundle</button>`;
  html += `<button type="button" class="btn glass sm" data-guest-action="refresh">Refresh guest intel</button>`;
  if (health.critical_services?.length) {
    health.critical_services.slice(0, 3).forEach((svc) => {
      html += `<button type="button" class="btn glass sm guest-intel-unit-btn" data-guest-action="restart-unit" data-unit="${escapeHtml(svc.name)}">Restart ${escapeHtml(svc.name)}</button>`;
      html += `<button type="button" class="btn glass sm guest-intel-unit-btn" data-guest-action="view-journal" data-unit="${escapeHtml(svc.name)}">Journal ${escapeHtml(svc.name)}</button>`;
    });
  }
  html += '</div>';

  const vm = state.selectedClusterVm;
  if (vm) {
    const pending = (state.pendingGuestActions || []).filter(
      (a) => a.namespace === vm.namespace && a.vm_name === vm.name && a.status === 'pending',
    );
    if (pending.length) {
      html += '<p class="finding warn"><strong>Pending approvals</strong></p><ul class="guest-intel-recs">';
      pending.forEach((a) => {
        html += `<li>${escapeHtml(a.action)} ${escapeHtml(a.unit || '')}${a.requested_by ? ` <span class="muted">by ${escapeHtml(a.requested_by)}</span>` : ''}
          <button type="button" class="btn glass sm accent" data-guest-approve="${escapeHtml(a.id)}">Approve</button>
          <button type="button" class="btn glass sm" data-guest-reject="${escapeHtml(a.id)}">Reject</button></li>`;
      });
      html += '</ul>';
    }
  }

  html += '</div>';
  container.innerHTML += html;
}

async function clusterGuestRestartUnit(unit) {
  const vm = state.selectedClusterVm;
  if (!vm || !unit) return;
  feed(`Restarting <strong>${escapeHtml(unit)}</strong> in guest…`);
  try {
    const data = await api(
      `/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}/guest/actions/restart-unit`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ unit }),
      },
    );
    const result = data.data;
    if (result?.status === 'pending_approval' && result?.action_id) {
      toast(`Restart queued for approval (${result.action_id})`, 'warn');
      await loadPendingGuestActions();
      return;
    }
    toast(`Restarted ${unit}`, 'ok');
    await fetchClusterGuestInfo(false);
  } catch (e) {
    toast(e.message, 'err');
  }
}

async function clusterGuestCollectBundle() {
  const vm = state.selectedClusterVm;
  if (!vm) return;
  feed('Collecting guest support bundle…');
  try {
    const data = await api(
      `/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}/guest/actions/collect-support-bundle`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: '{}',
      },
    );
    const result = data.data;
    if (result?.status === 'pending_approval' && result?.action_id) {
      toast(`Bundle collection queued for approval (${result.action_id})`, 'warn');
      await loadPendingGuestActions();
      return;
    }
    const bundle = result?.bundle;
    const inner = bundle?.result || bundle || result;
    showRaw({ mode: 'guest-support-bundle', bundle: inner });
    if (inner?.encoding === 'base64' && inner?.data) {
      const binary = Uint8Array.from(atob(inner.data), (c) => c.charCodeAt(0));
      const ext = inner.format === 'tar.zst' ? 'tar.zst' : 'json';
      const blob = new Blob([binary], { type: ext === 'tar.zst' ? 'application/zstd' : 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `${vm.namespace}-${vm.name}-support.${ext}`;
      a.click();
      URL.revokeObjectURL(url);
    }
    toast('Support bundle collected', 'ok');
  } catch (e) {
    toast(e.message, 'err');
  }
}

async function clusterGuestViewJournal(unit) {
  const vm = state.selectedClusterVm;
  if (!vm || !unit) return;
  feed(`Fetching journal for <strong>${escapeHtml(unit)}</strong>…`);
  try {
    const data = await api(
      `/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}/guest/journal?unit=${encodeURIComponent(unit)}&boot=current`,
    );
    showRaw({ mode: 'guest-journal', unit, slice: data.data });
    toast(`Journal loaded for ${unit}`, 'ok');
  } catch (e) {
    toast(e.message, 'err');
  }
}

function renderGuestControlPanel(container, control, doctor) {
  if (!control && !doctor) return;
  const caps = control?.capabilities || {};
  const data = control?.data || {};
  const transport = control?.transport || caps.transport || '—';
  const stateLabel = controlStateLabel(control?.controlState || caps.control_state);
  const stateClass = controlStateClass(control?.controlState || caps.control_state);
  let html = `<div class="guest-control-card glass-inset"><h3>Guest Control</h3>`;
  html += `<p class="finding ${stateClass}"><strong>${escapeHtml(stateLabel)}</strong></p>`;
  html += `<div class="guest-control-chips">`;
  html += `<span class="guest-intel-chip ${data.networkAvailable || caps.network ? 'ok' : 'warn'}">Network: ${data.networkAvailable || caps.network ? 'Available' : 'Unavailable'}</span>`;
  html += `<span class="guest-intel-chip ${data.qgaConnected || caps.qga ? 'ok' : 'warn'}">QGA: ${data.qgaConnected || caps.qga ? 'Connected' : 'Disconnected'}</span>`;
  html += `<span class="guest-intel-chip ${data.zyvorAgentInstalled || caps.zyvorAgent ? 'ok' : 'warn'}">Zyvor Agent: ${data.zyvorAgentInstalled || caps.zyvorAgent ? 'Installed' : 'Missing'}</span>`;
  html += `<span class="guest-intel-chip ${caps.pushTelemetry || caps.push_registered ? 'ok' : 'muted'}">Push: ${caps.pushTelemetry || caps.push_registered ? 'Enabled' : 'Disabled'}</span>`;
  html += `<span class="guest-intel-chip muted">Transport: ${escapeHtml(transport)}</span>`;
  if (control?.controlState === 'airgap_live') {
    html += `<span class="guest-intel-chip ok">Telemetry: Pull via virt-launcher</span>`;
  }
  html += `</div>`;
  const warnings = [...(control?.warnings || []), ...(caps.warnings || [])];
  if (warnings.length) {
    html += `<p class="finding warn"><strong>Warnings</strong> ${escapeHtml(warnings.join(' · '))}</p>`;
  }
  const actions = control?.recommendedActions || caps.recommended_actions || caps.recommendedActions || [];
  if (actions.length) {
    html += `<p class="finding ok"><strong>Recommended</strong> ${escapeHtml(actions.join(', '))}</p>`;
  }
  if (doctor?.data || doctor?.nodes) {
    const report = doctor.data || doctor;
    const nodes = report.nodes || [];
    if (nodes.length) {
      html += `<details class="guest-doctor-tree"><summary>Agent Doctor (${report.readinessScore ?? report.readiness_score ?? '—'}/100)</summary><ul class="guest-intel-recs">`;
      nodes.forEach((n) => {
        const cls = n.ok ? 'ok' : 'warn';
        html += `<li class="finding ${cls}">${escapeHtml(n.question)} → <strong>${escapeHtml(n.answer)}</strong></li>`;
      });
      html += `</ul></details>`;
    }
  }
  if (control?.controlState === 'disk_only') {
    html += `<p class="finding warn"><button type="button" class="btn glass sm" id="clusterGuestRepairBtn">Repair Guest Tools (offline)</button></p>`;
  }
  html += `</div>`;
  container.insertAdjacentHTML('beforeend', html);
  $('#clusterGuestRepairBtn')?.addEventListener('click', () => runClusterGuestRepairPlan());
}

async function runClusterGuestRepairPlan() {
  const vm = state.selectedClusterVm;
  if (!vm) return;
  feed(`Submitting offline guest repair plan for <strong>${escapeHtml(vm.namespace)}/${escapeHtml(vm.name)}</strong>…`);
  try {
    const res = await api(
      `/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}/guest/repair-plan`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          dryRun: true,
          injectQga: true,
          injectZyvorAgent: true,
          enableSystemd: true,
        }),
      },
    );
    const envelope = guestEnvelope(res);
    showRaw({ mode: 'guest-repair-plan', result: envelope.data || envelope });
    toast('Guest repair plan submitted', 'ok');
  } catch (e) {
    toast(e.message, 'err');
  }
}

function updateExecWarningBanner(control) {
  const el = $('#guestExecWarning');
  if (!el) return;
  const transport = control?.transport || '';
  const hostMediated = transport === 'qga-exec' || transport === 'qga-builtin' || control?.controlState === 'airgap_live';
  if (hostMediated) {
    el.classList.remove('hidden');
    el.textContent = 'Host-mediated exec: commands run via virt-launcher / QGA (guest network not required).';
  } else {
    el.classList.add('hidden');
  }
}

function setAgentControlStatus(control) {
  const el = $('#agentStatus');
  if (!el || !control) return;
  const label = controlStateLabel(control.controlState);
  const ok = control.controlState === 'full_agent' || control.controlState === 'airgap_live';
  el.textContent = label;
  el.className = 'agent-status' + (ok ? ' ok' : ' warn');
}

function renderClusterGuestSummary(info, bootInspect) {
  const ph = $('#summaryPlaceholder');
  const content = $('#summaryContent');
  if (!content) return; // summary panel absent in this layout
  ph?.classList.add('hidden');
  content.classList.remove('hidden');
  content.innerHTML = '';
  renderGuestControlPanel(content, state.lastGuestControlStatus, state.lastGuestDoctor);
  const healthClass = info.agent_connected ? 'ok' : info.health === 'absent' ? 'warn' : 'warn';
  content.innerHTML += `<p class="finding ${healthClass}"><strong>Guest agent:</strong> ${escapeHtml(info.health)} — ${escapeHtml(info.message)}</p>`;
  if (info.is_windows && !info.agent_connected) {
    content.innerHTML += `<p class="finding warn"><strong>Windows VM</strong> — install QEMU Guest Agent from virtio-win guest tools (not Linux cloud-init).</p>`;
  }
  if (info.os_name) {
    content.innerHTML += `<p class="finding ok"><strong>OS</strong> ${escapeHtml(info.os_name)} ${escapeHtml(info.os_version || '')}</p>`;
  }
  if (info.hostname) {
    content.innerHTML += `<p class="finding ok"><strong>Hostname</strong> ${escapeHtml(info.hostname)}</p>`;
  }
  if (info.interfaces?.length) {
    const ips = info.interfaces.map((i) => i.ipAddress).filter(Boolean).join(', ');
    if (ips) content.innerHTML += `<p class="finding ok"><strong>Interfaces</strong> ${escapeHtml(ips)}</p>`;
  }
  renderGuestIntelligenceCard(content, state.lastClusterGuestIntel);
  if (bootInspect) {
    const bootClass = bootInspect.available ? 'ok' : 'warn';
    const bootDetail = [
      bootInspect.os_release,
      bootInspect.bootloader,
      bootInspect.cloud_init_present ? 'cloud-init' : null,
      bootInspect.fstab_valid === false ? 'fstab issues' : null,
    ].filter(Boolean).join(' · ');
    content.innerHTML += `<p class="finding ${bootClass}"><strong>Boot inspect</strong> (${escapeHtml(bootInspect.source || 'spec')})${bootDetail ? ` — ${escapeHtml(bootDetail)}` : ''}</p>`;
    if (bootInspect.message) {
      content.innerHTML += `<p class="finding ${bootClass}">${escapeHtml(bootInspect.message)}</p>`;
    }
    if (bootInspect.available && bootInspect.source === 'guestkit') {
      content.innerHTML += `<p class="finding ok">GuestKit offline boot analysis completed on root disk.</p>`;
    }
  }
  if (state.lastClusterInspect) {
    renderGuestkitInspect(state.lastClusterInspect, content);
  }
  if (!info.agent_connected && info.vmi_running) {
    const installBtn = $('#clusterInstallAgentBtn');
    if (installBtn) {
      installBtn.disabled = false;
      installBtn.classList.toggle('hidden', false);
      installBtn.querySelector('strong').textContent = info.is_windows ? 'Guest Tools' : 'Install VM Tools';
      installBtn.querySelector('span').textContent = info.is_windows
        ? 'virtio-win in Zeus OS'
        : 'Zeus guest agent';
    }
    if (info.is_windows) {
      content.innerHTML += `<p class="finding warn">Open <strong>Zeus OS → Guest Tools</strong>, attach virtio-win.iso, install QEMU Guest Agent, then restart.</p>`;
    } else {
      content.innerHTML += `<p class="finding warn">Click <strong>Install agent</strong> to merge GuestKit cloud-init, or use Zeus OS Guest Tools.</p>`;
    }
  } else {
    const installBtn = $('#clusterInstallAgentBtn');
    if (installBtn) installBtn.disabled = true;
  }
  showRaw({ mode: 'kubevirt-live', guest_agent: info, boot_inspect: bootInspect || null });
  const vm = state.selectedClusterVm;
  if (vm) {
    renderClusterDetailDrawer(vm, info, state.lastClusterGuestIntel);
    updateVmtoolsLifecycleButtons(vm, info);
    renderClusterCopilot(info, vm, bootInspect).catch((e) => {
      console.error('Cluster Copilot render failed:', e);
      feed(`Ask Zeus briefing failed: ${escapeHtml(e.message)}`, 'err');
    });
  }
}

async function clusterVmtoolsOp(action) {
  const vm = state.selectedClusterVm;
  if (!vm) {
    toast('Select a cluster VM first', 'err');
    return;
  }
  const labels = {
    quiesce: 'Quiescing filesystem',
    unquiesce: 'Thawing filesystem',
    reboot: 'Guest soft reboot',
    shutdown: 'Guest shutdown',
  };
  feed(`${labels[action] || action} on <strong>${escapeHtml(vm.namespace)}/${escapeHtml(vm.name)}</strong>…`);
  try {
    const opts = action === 'shutdown'
      ? { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ grace_period_seconds: 60 }) }
      : { method: 'POST' };
    const data = await api(
      `/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}/vmtools/${action}`,
      opts,
    );
    toast(data.data.message || `${action} complete`, 'ok');
    feed(data.data.message || `${action} finished`, 'ok');
    await loadClusterFleet();
    if (action !== 'shutdown') await fetchClusterGuestInfo(false);
  } catch (e) {
    toast(e.message, 'err');
  }
}

async function clusterLifecycle(action) {
  const vm = state.selectedClusterVm;
  if (!vm) {
    toast('Select a cluster VM first', 'err');
    return;
  }
  feed(`${action} <strong>${escapeHtml(vm.namespace)}/${escapeHtml(vm.name)}</strong>…`);
  try {
    const data = await api(
      `/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}/${action}`,
      { method: 'PUT' },
    );
    const result = data.data;
    toast(result.message || `${action} requested`, 'ok');
    feed(result.message || `${action} complete`, 'ok');
    await loadClusterFleet();
    if (state.selectedClusterVm) {
      updateClusterLifecycleButtons(state.selectedClusterVm);
      if (action !== 'stop') await fetchClusterGuestInfo(false);
    }
  } catch (e) {
    toast(e.message, 'err');
  }
}

async function stopThenBootInspect() {
  const vm = state.selectedClusterVm;
  if (!vm) {
    toast('Select a cluster VM first', 'err');
    return;
  }
  feed(`Stopping <strong>${escapeHtml(vm.namespace)}/${escapeHtml(vm.name)}</strong> for boot inspect…`);
  try {
    await api(
      `/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}/stop`,
      { method: 'PUT' },
    );
    for (let i = 0; i < 60; i++) {
      await new Promise((r) => setTimeout(r, 1000));
      await loadClusterFleet();
      const phase = String(state.selectedClusterVm?.phase || '').toLowerCase();
      if (!phase.includes('run')) break;
    }
    await fetchClusterBootInspect(true);
  } catch (e) {
    toast(e.message, 'err');
  }
}

async function exportClusterDisk(forceStop = false) {
  const vm = state.selectedClusterVm;
  if (!vm) {
    toast('Select a cluster VM first', 'err');
    return;
  }
  feed(`Exporting root disk from <strong>${escapeHtml(vm.namespace)}/${escapeHtml(vm.name)}</strong>…`);
  try {
    const qs = forceStop ? '?force_stop=true' : '';
    const data = await api(
      `/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}/export-disk${qs}`,
      { method: 'POST' },
    );
    const result = data.data;
    toast(`Exported as ${result.name}`, 'ok');
    feed(`Disk exported — <strong>${escapeHtml(result.name)}</strong> (${fmtBytes(result.size_bytes)})`, 'ok');
    setFleetMode('disks');
    await loadFleet();
    const imported = state.vms.find((v) => v.id === result.vm_id);
    if (imported) {
      selectVm(imported);
      setInspectionMode('offline');
      feed('Continue with offline <strong>Doctor → Migrate → Provision</strong>', 'ok');
    }
  } catch (e) {
    if (!forceStop && String(e.message).toLowerCase().includes('running')) {
      if (window.confirm('VM is running. Stop it and export the root disk?')) {
        await exportClusterDisk(true);
      }
      return;
    }
    toast(e.message, 'err');
  }
}

async function applyYamlToCluster(yaml) {
  const content = yaml || state.lastYaml;
  if (!content) {
    toast('Generate YAML first', 'err');
    return;
  }
  const kinds = (content.match(/^kind:/gm) || []).length;
  if (!window.confirm(`Apply ${kinds || 'multi-doc'} KubeVirt resource(s) to the cluster?`)) return;
  feed('Applying manifests to cluster…');
  try {
    const data = await api('/kubevirt/apply', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ yaml: content }),
    });
    const result = data.data;
    const summary = (result.resources || []).map((r) => `${r.action} ${r.kind}/${r.name}`).join(', ');
    toast(result.applied ? 'Manifests applied' : 'Apply completed with errors', result.applied ? 'ok' : 'err');
    feed(summary || result.errors?.join('; ') || 'Apply finished', result.applied ? 'ok' : 'err');
    if (result.resources?.length) {
      const first = result.resources.find((r) => r.kind === 'VirtualMachine') || result.resources[0];
      if (first?.namespace && first?.name) {
        const url = new URL(window.location.href);
        url.searchParams.set('namespace', first.namespace);
        url.searchParams.set('vm', first.name);
        url.searchParams.set('action', 'inspect');
        history.replaceState(null, '', url);
        setFleetMode('cluster');
        await loadClusterFleet();
        const clusterVm = state.clusterVms.find((v) => v.namespace === first.namespace && v.name === first.name);
        if (clusterVm) selectClusterVm(clusterVm);
      }
    }
  } catch (e) {
    toast(e.message, 'err');
  }
}

async function runVmToolsDiagnostics() {
  const vm = state.selectedClusterVm;
  if (!vm) {
    toast('Select a cluster VM first', 'err');
    return;
  }
  feed(`Running VM Tools diagnostics on <strong>${escapeHtml(vm.namespace)}/${escapeHtml(vm.name)}</strong>…`);
  try {
    const data = await api(
      `/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}/vmtools/diagnostics`,
      { method: 'POST' },
    );
    toast(data.data.message || 'Diagnostics complete', 'ok');
    feed(data.data.message || 'VM Tools diagnostics finished');
    await fetchClusterGuestInfo(false);
  } catch (e) {
    toast(e.message, 'err');
  }
}

async function installClusterGuestAgent(method = 'auto') {
  const vm = state.selectedClusterVm;
  if (!vm) {
    toast('Select a cluster VM first', 'err');
    return;
  }
  if (vm.is_windows) {
    window.open(zeusVmUrl(vm.namespace, vm.name, 'guest'), '_blank', 'noopener,noreferrer');
    return;
  }
  const label = method === 'iso' ? 'Attaching VM Tools ISO' : 'Installing Zeus VM Tools';
  feed(`${label} on ${escapeHtml(vm.namespace)}/${escapeHtml(vm.name)}…`);
  try {
    const airgap = state.lastGuestControlStatus?.controlState === 'airgap_live'
      || state.lastGuestControlStatus?.controlState === 'qga_only';
    const endpoint = airgap
      ? `/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}/guest/install-agent`
      : `/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}/vmtools/install?restart=true&method=${encodeURIComponent(method)}`;
    const options = airgap
      ? {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ strategy: 'auto', restart: true }),
        }
      : { method: 'POST' };
    const data = await api(endpoint, options);
    const envelope = guestEnvelope(data);
    const result = envelope.data || data.data || data;
    toast(result.message || (result.success ? 'Install started' : 'Install failed'), result.success !== false ? 'ok' : 'err');
    feed(result.next_steps?.join(' ') || result.message);
    if (result.bootstrap_script) {
      feed(`<pre class="raw-json">${escapeHtml(result.bootstrap_script)}</pre>`);
      try {
        await navigator.clipboard.writeText(result.bootstrap_script);
        toast('Bootstrap script copied to clipboard', 'ok');
      } catch {
        /* clipboard optional */
      }
    }
    if (result.success) {
      setTimeout(() => fetchClusterGuestInfo(false), 8000);
      await loadClusterFleet();
    }
  } catch (e) {
    toast(e.message, 'err');
  }
}

function setDockEnabled(on) {
  ['#dockInspect', '#dockDoctor', '#dockPlan', '#dockRepair', '#dockLaunch'].forEach((sel) => {
    const el = $(sel);
    if (el) el.disabled = !on;
  });
}

function clusterVmStopped(vm) {
  if (!vm) return false;
  const phase = String(vm.phase || vm.status || '').toLowerCase();
  return !phase.includes('run');
}

function setClusterDockEnabled(on) {
  const vm = state.selectedClusterVm;
  const stopped = on && clusterVmStopped(vm);
  $('#dockInspect')?.toggleAttribute('disabled', !on);
  $('#dockDoctor')?.toggleAttribute('disabled', !on);
  ['#dockPlan', '#dockRepair', '#dockLaunch'].forEach((sel) => {
    const el = $(sel);
    if (el) el.disabled = !stopped;
  });
}

function selectVm(vm) {
  state.selectedVm = vm;
  state.selectedClusterVm = null;
  setInspectionMode('offline');
  renderFleet();
  setAgentDeckEnabled(!!vm);
  const cache = getVmCache(vm.id);
  const failed = cache.status === 'failed';
  $('#selectedVmTitle').textContent = vm.name || 'Unnamed disk';
  const meta = $('#selectedVmMeta');
  const smoke = isSmokeDisk(vm);
  meta.textContent = `${vm.format} · ${fmtBytes(vm.size_bytes)} · ${vm.id}`;
  meta.classList.toggle('vm-warn', smoke || failed);
  if (smoke) {
    meta.textContent += ' — placeholder disk; upload or select a real cloud image';
  } else if (failed && cache.lastError) {
    meta.textContent += ` — ${cache.lastError}`;
  }
  $('#jobTracker')?.classList.add('hidden');
  window.GuestKitConsole?.syncBrainJobTracker?.();
  window.GuestKitConsole?.resetJobBadge?.();
  $$('.action-card').forEach((b) => { b.disabled = false; });
  setDockEnabled(true);
  if (state.wizard.step === 'ingest' && state.wizard.completed.has('ingest')) {
    setWizardStep('assure');
  }
  updateWizardFooter();
  updateSelectionPanels();
  updateCopilotPlaceholder();
  window.GuestKitConsole?.onSelectVmConsole?.();
  window.GuestKitFeatures?.loadJobHistory?.(vm.id);
  window.GuestKitNebula?.renderAllNebula?.(vm, cache);
  feed(`Selected <strong>${escapeHtml(vm.name)}</strong>${smoke ? ' (smoke — not bootable)' : ''}`, smoke ? 'err' : '');
}

async function loadFleet() {
  try {
    const data = await api('/vms');
    state.vms = data.data || [];
    if (state.selectedVm) {
      const fresh = state.vms.find((v) => v.id === state.selectedVm.id);
      if (fresh) state.selectedVm = fresh;
      else state.selectedVm = null;
    }
    if (!state.selectedVm || isSmokeDisk(state.selectedVm)) {
      const best = pickBestVm(state.vms);
      if (best && best.id !== state.selectedVm?.id) selectVm(best);
      else renderFleet();
    } else {
      renderFleet();
    }
    window.GuestKitConsole?.refreshHudStatus?.();
  } catch (e) {
    toast(`Fleet load failed: ${e.message}`, 'err');
  }
}

function setUploadProgress(pct, label) {
  const wrap = $('#uploadProgress');
  const bar = $('#uploadBar');
  wrap.classList.toggle('hidden', pct < 0);
  if (pct >= 0) {
    bar.style.setProperty('--pct', `${pct}%`);
    $('#uploadLabel').textContent = label;
  }
}

function uploadWithProgress(file) {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    const form = new FormData();
    form.append('file', file);

    xhr.upload.addEventListener('progress', (e) => {
      if (e.lengthComputable) {
        const pct = Math.round((e.loaded / e.total) * 100);
        setUploadProgress(pct, `Uploading ${file.name}… ${pct}%`);
      }
    });

    xhr.addEventListener('load', () => {
      let data = {};
      try { data = JSON.parse(xhr.responseText); } catch { /* */ }
      if (xhr.status >= 200 && xhr.status < 300) resolve(data);
      else reject(new Error(data.message || data.error || xhr.statusText));
    });

    xhr.addEventListener('error', () => reject(new Error('Upload failed')));
    xhr.open('POST', `${API_BASE}/vms/import`);
    xhr.send(form);
  });
}

async function uploadFile(file) {
  if (!file) return;
  setUploadProgress(0, `Uploading ${file.name}…`);
  feed(`Ingesting <strong>${escapeHtml(file.name)}</strong>…`);

  try {
    const data = await uploadWithProgress(file);
    setUploadProgress(100, 'Complete');
    const vm = data.data;
    toast(`Ingested ${vm.name}`, 'ok');
    feed(`Ingest complete — <strong>${escapeHtml(vm.name)}</strong>`, 'ok');
    markWizardComplete('ingest');
    await loadFleet();
    selectVm(vm);
    window.GuestKitConsole?.appendTimelineEvent?.(vm.id, 'uploaded', vm.name);
    setWizardStep('assure');
    setTimeout(() => setUploadProgress(-1), 1200);
  } catch (e) {
    setUploadProgress(-1);
    toast(e.message, 'err');
    feed(`Ingest failed: ${escapeHtml(e.message)}`, 'err');
  }
}

function setupDropzone() {
  const zone = $('#dropzone');
  const input = $('#fileInput');

  zone.addEventListener('click', (e) => {
    if (e.target.id !== 'browseBtn' && e.target.closest('#browseBtn')) return;
    if (e.target.id === 'browseBtn' || e.target === zone || e.target.closest('.dropzone-inner')) {
      input.click();
    }
  });

  $('#browseBtn').addEventListener('click', (e) => {
    e.stopPropagation();
    input.click();
  });

  input.addEventListener('change', () => {
    if (input.files[0]) uploadFile(input.files[0]);
    input.value = '';
  });

  ['dragenter', 'dragover'].forEach((ev) => {
    zone.addEventListener(ev, (e) => {
      e.preventDefault();
      zone.classList.add('dragover');
    });
  });

  ['dragleave', 'drop'].forEach((ev) => {
    zone.addEventListener(ev, (e) => {
      e.preventDefault();
      zone.classList.remove('dragover');
    });
  });

  zone.addEventListener('drop', (e) => {
    const file = e.dataTransfer?.files?.[0];
    if (file) uploadFile(file);
  });
}

function showJobTracker(op, jobId) {
  clearTimeout(state.jobTrackerHideTimer);
  state.activeJob = { id: jobId, op, start: Date.now() };
  const tracker = $('#jobTracker');
  const bar = $('#jobBar');
  if (!tracker || !bar) { window.GuestKitConsole?.syncBrainJobTracker?.(); return; } // job tracker absent in this layout
  tracker.classList.remove('hidden');
  tracker.style.display = '';
  $('#jobOp').textContent = op.replace(/-/g, ' ');
  $('#jobIdDisplay').textContent = jobId;
  $('#jobStatus').textContent = 'pending';
  $('#jobStatus').className = 'job-status';
  bar.className = 'job-bar-fill';
  bar.style.width = '8%';
  $('#jobBadge').textContent = 'running';
  $('#jobBadge').className = 'badge live running';
  $('#jobRetryBtn')?.classList.add('hidden');
  window.GuestKitConsole?.syncBrainJobTracker?.();
}

function hideJobTracker(status) {
  const bar = $('#jobBar');
  state.activeJob = null;
  if (!bar) { window.GuestKitConsole?.syncBrainJobTracker?.(); return; } // job tracker absent in this layout
  bar.classList.add(status === 'failed' ? 'fail' : 'done');
  if (status === 'completed') bar.style.width = '100%';
  $('#jobStatus').textContent = status;
  $('#jobStatus').className = `job-status ${status}`;
  $('#jobBadge').textContent = status;
  $('#jobBadge').className = `badge live ${status === 'completed' ? 'done' : 'fail'}`;
  window.GuestKitConsole?.syncBrainJobTracker?.();
  const hideMs = status === 'failed' ? 6000 : 3500;
  clearTimeout(state.jobTrackerHideTimer);
  state.jobTrackerHideTimer = setTimeout(() => {
    $('#jobTracker')?.classList.add('hidden');
    window.GuestKitConsole?.syncBrainJobTracker?.();
    window.GuestKitConsole?.resetJobBadge?.();
  }, hideMs);
}

function setJobProgress(pct, message) {
  const bar = $('#jobBar');
  if (!bar) return; // job tracker absent in this layout
  if (pct != null) {
    bar.classList.add('progress');
    bar.style.width = `${Math.max(8, Math.min(100, pct))}%`;
  }
  if (message) $('#jobStatus').textContent = message;
}

function setScore(score, reason) {
  const panel = $('#scorePanel');
  const ring = $('#scoreRing');
  const val = $('#scoreValue');
  if (!panel || !ring || !val) return; // score widget absent in this layout
  const label = panel.querySelector('.score-label');

  if (score == null || Number.isNaN(score)) {
    if (reason) {
      panel.classList.remove('hidden');
      val.textContent = '—';
      ring.style.strokeDashoffset = 327;
      ring.style.stroke = 'var(--text-tertiary)';
      if (label) label.textContent = reason;
    } else {
      panel.classList.add('hidden');
    }
    return;
  }

  panel.classList.remove('hidden');
  if (label) label.textContent = 'boot';
  const pct = Math.max(0, Math.min(100, score));
  val.textContent = Math.round(pct);
  const offset = 327 - (327 * pct) / 100;
  ring.style.strokeDashoffset = offset;

  if (pct >= 75) ring.style.stroke = 'var(--success)';
  else if (pct >= 50) ring.style.stroke = 'var(--warn)';
  else ring.style.stroke = 'var(--danger)';
}

function normalizeAgentPayload(payload) {
  if (!payload || typeof payload !== 'object') return payload;
  if (payload.inspect) {
    return {
      mode: 'offline',
      inspect: payload.inspect,
      summary: payload.summary?.image
        ? `GuestKit inspect — ${payload.summary.image}`
        : 'GuestKit disk inspect',
    };
  }
  if (payload.boot_report && !payload.bootability) {
    return {
      ...payload,
      bootability: payload.boot_report,
      mode: 'online',
      summary: payload.boot_report?.summary || 'Live agent doctor',
    };
  }
  if (payload.schema_version || payload.os || payload.collected_at) {
    const os = payload.os || {};
    return {
      mode: 'online',
      evidence: payload,
      summary: `Live evidence — ${os.distribution || os.os_type || 'guest'} ${os.version || ''}`.trim(),
    };
  }
  if (payload.methods || payload.fix_apply != null) {
    return { mode: 'online', capabilities: payload, summary: 'Agent capabilities' };
  }
  return { ...payload, mode: payload.mode || 'online' };
}

function extractPayload(data) {
  const jobResult = data?.data?.result;
  let payload;
  if (jobResult) {
    payload = jobResult.data
      ? jobResult.data
      : (typeof jobResult === 'string' ? tryParse(jobResult) : jobResult);
  } else {
    const alt = data?.data?.live_status?.result || data?.result;
    payload = typeof alt === 'string' ? tryParse(alt) : alt;
  }
  return normalizeAgentPayload(payload);
}

function isOnlineMode() {
  return state.inspectionMode === 'online';
}

function getAgentProxyUrl() {
  const input = $('#agentProxyUrl')?.value?.trim();
  return input || state.agentProxyUrl || '';
}

function setAgentStatus(text, ok) {
  const el = $('#agentStatus');
  if (!el) return;
  el.textContent = text;
  el.className = 'agent-status' + (ok === true ? ' ok' : ok === false ? ' err' : '');
}

function setInspectionMode(mode) {
  state.inspectionMode = mode;
  localStorage.setItem('zyvor.inspectMode', mode);
  const offline = mode === 'offline';
  $('#modeOfflineBtn')?.classList.toggle('active', offline);
  $('#modeOnlineBtn')?.classList.toggle('active', !offline);
  $('#agentProxyRow')?.classList.toggle('hidden', offline);
  $('#actionDeck')?.classList.toggle('hidden', !offline);
  $('#agentDeck')?.classList.toggle('hidden', offline);
  $('#agentTab')?.classList.toggle('hidden', offline);
  if (!offline) {
    setActiveTab(state.selectedClusterVm ? 'summary' : 'agent');
    renderAgentChips();
  }
  updateSelectionPanels();
  feed(`Inspection mode: <strong>${offline ? 'offline disk' : 'online agent'}</strong>`);
}

function setAgentDeckEnabled(on) {
  $$('#agentDeck [data-agent-action]').forEach((b) => { b.disabled = !on; });
}

function renderFinding(item, kind) {
  if (typeof item === 'string') {
    const icon = kind === 'blocker' ? '⛔' : '⚠';
    return `<p class="finding ${kind}">${icon} ${escapeHtml(item)}</p>`;
  }
  const title = item.title ? `<strong>${escapeHtml(item.title)}</strong>: ` : '';
  const msg = escapeHtml(item.message || JSON.stringify(item));
  const remed = item.remediation
    ? `<br><span class="remediation">→ ${escapeHtml(item.remediation)}</span>`
    : '';
  const icon = kind === 'blocker' ? '⛔' : '⚠';
  return `<p class="finding ${kind}">${icon} ${title}${msg}${remed}</p>`;
}

function renderGuestkitInspect(inspect, contentEl) {
  const content = contentEl || $('#summaryContent');
  if (!inspect || !content) return;
  const os = inspect.operating_system || {};
  const osLine = [
    os.distribution || os.product_name || os.type,
    os.version,
    os.arch,
  ].filter(Boolean).join(' · ');
  if (osLine) {
    content.innerHTML += `<p class="finding ok"><strong>GuestKit inspect</strong> — ${escapeHtml(osLine)}${os.hostname ? ` · ${escapeHtml(os.hostname)}` : ''}</p>`;
  } else {
    content.innerHTML += `<p class="finding ok"><strong>GuestKit inspect</strong> — disk analysis complete</p>`;
  }
  if (inspect.packages?.count != null) {
    const mgr = inspect.packages.manager ? ` (${inspect.packages.manager})` : '';
    content.innerHTML += `<p class="finding ok"><strong>Packages</strong> ${inspect.packages.count}${escapeHtml(mgr)}</p>`;
    (inspect.packages.sample || []).slice(0, 8).forEach((pkg) => {
      const label = typeof pkg === 'string' ? pkg : (pkg.name || JSON.stringify(pkg));
      content.innerHTML += `<p class="finding ok">→ ${escapeHtml(label)}</p>`;
    });
    if (inspect.packages.count > 8) {
      content.innerHTML += `<p class="finding ok">…and ${inspect.packages.count - 8} more (see Raw tab)</p>`;
    }
  }
  if (inspect.services?.count != null) {
    content.innerHTML += `<p class="finding ok"><strong>Enabled services</strong> ${inspect.services.count}</p>`;
    (inspect.services.sample || []).slice(0, 6).forEach((svc) => {
      content.innerHTML += `<p class="finding ok">→ ${escapeHtml(String(svc))}</p>`;
    });
  }
  if (inspect.network?.hostname) {
    content.innerHTML += `<p class="finding ok"><strong>Network hostname</strong> ${escapeHtml(inspect.network.hostname)}</p>`;
  }
  if (inspect.security) {
    const sel = inspect.security.selinux?.enabled ? 'SELinux on' : 'SELinux off';
    const aa = inspect.security.apparmor?.enabled ? 'AppArmor on' : 'AppArmor off';
    content.innerHTML += `<p class="finding ok"><strong>Security</strong> ${sel} · ${aa}</p>`;
  }
  if (inspect.mountpoints?.count != null) {
    content.innerHTML += `<p class="finding ok"><strong>Mount points</strong> ${inspect.mountpoints.count}</p>`;
  }
}

function renderSummary(data, action) {
  const ph = $('#summaryPlaceholder');
  const content = $('#summaryContent');
  const emptyWrap = $('#summaryEmpty');
  if (!content) return; // legacy summary panel removed in the nebula layout
  ph?.classList.add('hidden');
  emptyWrap?.classList.add('hidden');
  content.classList.remove('hidden');
  content.innerHTML = '';

  if (action === 'provision' && data?.data?.yaml) {
    content.innerHTML = `<p class="finding ok">KubeVirt manifests generated — <strong>${(data.data.yaml.match(/^kind:/gm) || []).length || 2}</strong> resources ready for CDI import.</p>`;
    setScore(null);
    return;
  }

  const payload = extractPayload(data);
  const boot = payload?.bootability;
  const migrate = payload?.migration_score;
  const repair = payload?.fix_plan || (payload?.message && payload?.before_score != null ? payload : null);

  if (boot?.score != null) setScore(boot.score);
  else if (migrate?.score != null) setScore(migrate.score);
  else if (payload?.before_score != null) setScore(payload.before_score);
  else setScore(null);

  if (payload?.mode === 'online') {
    content.innerHTML += `<p class="finding ok">🟢 <strong>Online agent</strong> — live inspection from running guest</p>`;
  } else if (payload?.mode === 'offline' && payload?.inspect) {
    content.innerHTML += `<p class="finding ok">💾 <strong>Offline GuestKit</strong> — disk inspect from image</p>`;
  }

  if (payload?.inspect) {
    renderGuestkitInspect(payload.inspect, content);
  }

  if (payload?.evidence?.os) {
    const os = payload.evidence.os;
    content.innerHTML += `<p class="finding ok"><strong>${escapeHtml(os.distribution || os.os_type || 'guest')}</strong> ${escapeHtml(os.version || '')} · ${escapeHtml(os.architecture || '')} · ${escapeHtml(os.hostname || '')}</p>`;
  }

  if (payload?.capabilities?.methods) {
    content.innerHTML += `<p class="finding ok">Agent methods: ${payload.capabilities.methods.map((m) => escapeHtml(m)).join(', ')}</p>`;
  }

  if (boot?.summary) {
    content.innerHTML += `<p class="finding ok">${escapeHtml(boot.summary)}</p>`;
  }

  (boot?.blockers || []).forEach((b) => {
    content.innerHTML += renderFinding(b, 'blocker');
  });

  (boot?.warnings || []).forEach((w) => {
    content.innerHTML += renderFinding(w, 'warn');
  });

  if (migrate) {
    if (migrate.driver_injections?.length) {
      content.innerHTML += `<p><strong>Driver injections</strong></p>`;
      migrate.driver_injections.forEach((d) => {
        content.innerHTML += `<p class="finding ok">→ ${escapeHtml(d)}</p>`;
      });
    }
    if (migrate.required_changes?.length) {
      content.innerHTML += `<p><strong>Required changes</strong></p>`;
      migrate.required_changes.forEach((c) => {
        content.innerHTML += `<p class="finding warn">→ ${escapeHtml(c)}</p>`;
      });
    }
    if (migrate.licensing_warnings?.length) {
      migrate.licensing_warnings.forEach((w) => {
        content.innerHTML += `<p class="finding warn">⚠ ${escapeHtml(w)}</p>`;
      });
    }
    if (migrate.estimated_downtime_minutes != null) {
      content.innerHTML += `<p class="finding ok">Est. downtime: <strong>${migrate.estimated_downtime_minutes} min</strong></p>`;
    }
  }

  if (payload?.root_cause) {
    const rc = payload.root_cause;
    content.innerHTML += `<p><strong>Root cause</strong> (${Math.round((rc.confidence || 0) * 100)}% confidence)</p>`;
    if (rc.summary) content.innerHTML += `<p class="finding ok">${escapeHtml(rc.summary)}</p>`;
    if (rc.primary_cause) content.innerHTML += `<p class="finding warn">${escapeHtml(rc.primary_cause)}</p>`;
    (rc.chain || []).forEach((step) => {
      content.innerHTML += `<p class="finding ok">${step.step}. ${escapeHtml(step.description)}</p>`;
    });
  }

  if (repair?.fix_plan?.operations?.length) {
    const fp = repair.fix_plan;
    content.innerHTML += `<p><strong>Fix plan</strong> — ${fp.operations.length} operation(s), risk: ${escapeHtml(fp.overall_risk || 'unknown')}</p>`;
    fp.operations.slice(0, 5).forEach((op) => {
      const name = op.name || op.kind || op.id || 'operation';
      content.innerHTML += `<p class="finding ok">→ ${escapeHtml(name)}</p>`;
    });
    if (fp.operations.length > 5) {
      content.innerHTML += `<p class="finding ok">…and ${fp.operations.length - 5} more</p>`;
    }
  } else if (payload?.message) {
    content.innerHTML += `<p class="finding ok">${escapeHtml(payload.message)}</p>`;
    if (payload.after_score != null) {
      content.innerHTML += `<p class="finding ok">Projected score: <strong>${Math.round(payload.after_score)}</strong></p>`;
    }
  }

  const err = data?.data?.live_status?.error || data?.data?.result?.error?.message;
  if (err) {
    const friendly = humanizeJobError(err, state.selectedVm);
    content.innerHTML += `<p class="finding blocker">${escapeHtml(friendly)}</p>`;
    const alt = pickBestVm(state.vms.filter((v) => v.id !== state.selectedVm?.id));
    if (alt) {
      content.innerHTML += `<p class="finding warn">Try <strong>${escapeHtml(alt.name)}</strong> (${fmtBytes(alt.size_bytes)}) — <button type="button" class="linkish" id="summarySwitchDiskBtn">switch disk</button> and run Inspect.</p>`;
      content.querySelector('#summarySwitchDiskBtn')?.addEventListener('click', () => selectVm(alt));
    }
    setScore(null, 'failed');
  }

  renderChecksHeatmap(boot?.checks);
  renderCopilot(payload?.copilot, payload);
  window.GuestKitConsole?.renderBrainPanel?.();

  if (!content.innerHTML) {
    ph.classList.remove('hidden');
    content.classList.add('hidden');
    ph.textContent = 'Run an action to see results.';
  }
}

function renderChecksHeatmap(checks) {
  const el = $('#checksHeatmap');
  if (!checks?.length) {
    el.classList.add('hidden');
    el.innerHTML = '';
    return;
  }
  el.classList.remove('hidden');
  el.innerHTML = '<p class="checks-label">Boot checks</p><div class="checks-grid">' + checks.map((c) => {
    const cls = c.passed ? 'pass' : (c.severity === 'Blocker' || c.severity === 'blocker' ? 'fail' : 'warn');
    return `<span class="check-cell ${cls}" title="${escapeHtml(c.message)}">${escapeHtml(c.id)}</span>`;
  }).join('') + '</div>';
}

function readinessClass(r) {
  return { ready: 'ok', caution: 'warn', blocked: 'err', high_risk: 'err' }[r] || '';
}

function renderCopilot(briefing, payload) {
  const tab = $('#copilotTab');
  const ph = $('#copilotPlaceholder');
  const brief = $('#copilotBrief');
  const chips = $('#copilotChips');
  const mini = $('#copilotMini');

  if (!briefing) {
    tab.classList.add('hidden');
    ph.classList.remove('hidden');
    brief.classList.add('hidden');
    chips.innerHTML = '';
    mini.classList.add('hidden');
    state.lastBriefing = null;
    $('#copilotBanner').classList.add('hidden');
    window.GuestKitConsole?.renderBrainPanel?.();
    return;
  }

  state.lastBriefing = briefing;
  tab.classList.remove('hidden');
  ph.classList.add('hidden');
  brief.classList.remove('hidden');

  const digest = briefing.evidence_digest || payload?.evidence_digest;
  brief.innerHTML = `
    <div class="copilot-head row">
      <span class="readiness-pill ${readinessClass(briefing.readiness)}">${escapeHtml(briefing.readiness)}</span>
      <span class="copilot-score">${Math.round(briefing.boot_score)} boot${briefing.migration_score != null ? ` · ${Math.round(briefing.migration_score)} migrate` : ''}</span>
    </div>
    <h3 class="copilot-headline">${escapeHtml(briefing.headline)}</h3>
    <p class="copilot-summary">${escapeHtml(briefing.summary)}</p>
    ${digest ? `<p class="copilot-digest mono">${escapeHtml(digest.os)} · ${escapeHtml(digest.architecture)} · ${escapeHtml(digest.bootloader)}</p>` : ''}
    ${(briefing.evidence_highlights || []).map((h) => `
      <div class="evidence-highlight">
        <span class="evidence-ref">${escapeHtml(h.ref)}</span>
        <strong>${escapeHtml(h.label)}</strong>
        <p>${escapeHtml(h.detail)}</p>
      </div>
    `).join('')}
    ${(briefing.recommended_actions || []).slice(0, 3).map((a) => `
      <button type="button" class="copilot-action" data-workflow="${escapeHtml(a.workflow)}">
        <span class="copilot-action-pri">#${a.priority}</span>
        <span><strong>${escapeHtml(a.title)}</strong><br>${escapeHtml(a.detail)}</span>
      </button>
    `).join('')}
  `;

  brief.querySelectorAll('.copilot-action').forEach((btn) => {
    btn.addEventListener('click', () => runWorkflow(btn.dataset.workflow));
  });

  chips.innerHTML = '';
  (briefing.insights || []).forEach((ins) => {
    const chip = document.createElement('button');
    chip.type = 'button';
    chip.className = 'copilot-chip';
    chip.textContent = ins.question;
    chip.addEventListener('click', () => appendCopilotMessage(ins.question, ins.answer));
    chips.appendChild(chip);
  });

  mini.classList.remove('hidden');
  $('#copilotMiniHeadline').textContent = briefing.headline;
  const pill = $('#copilotReadiness');
  pill.textContent = briefing.readiness;
  pill.className = `readiness-pill ${readinessClass(briefing.readiness)}`;

  showCopilotBanner(briefing);
  window.GuestKitConsole?.renderBrainPanel?.();
}

function showCopilotBanner(briefing) {
  const banner = $('#copilotBanner');
  const action = briefing.recommended_actions?.[0];
  $('#copilotBannerTitle').textContent = briefing.headline;
  $('#copilotBannerMsg').textContent = action
    ? `${action.title} — ${action.detail}`
    : briefing.summary;
  const btn = $('#copilotBannerAction');
  btn.textContent = workflowLabel(briefing.next_workflow);
  btn.dataset.action = briefing.next_workflow;
  banner.classList.remove('hidden');
}

function workflowLabel(wf) {
  const labels = {
    'repair-plan': 'Run Repair',
    'migration-plan': 'Run Migrate',
    provision: 'Generate YAML',
    doctor: 'Run Doctor',
    passport: 'Download Passport',
    'cluster-install-agent': 'Install VM Tools',
    'cluster-guest-info': 'Refresh guest info',
    'cluster-boot-inspect': 'Boot inspect',
    'cluster-zeus': 'Open in Zeus',
  };
  return labels[wf] || 'Next step';
}

function appendCopilotMessage(question, answer, fromUser = true) {
  const chat = $('#copilotChat');
  if (fromUser) {
    const u = document.createElement('div');
    u.className = 'copilot-msg user';
    u.textContent = question;
    chat.appendChild(u);
  }
  const a = document.createElement('div');
  a.className = 'copilot-msg assistant';
  a.innerHTML = escapeHtml(answer).replace(/\n/g, '<br>');
  chat.appendChild(a);
  chat.scrollTop = chat.scrollHeight;
}

function answerCopilotLocal(question) {
  const b = state.selectedClusterVm ? state.lastClusterBriefing : state.lastBriefing;
  if (!b?.insights?.length) {
    return {
      answer: state.selectedClusterVm
        ? 'Fetch guest info on the selected cluster VM to build a live Ask Zeus briefing.'
        : 'Run Doctor first to build a migration briefing.',
    };
  }

  const q = question.toLowerCase();
  let id = 'boot_score';
  if (q.includes('agent') || q.includes('connect')) id = 'agent_status';
  else if (q.includes('block') || q.includes('stop')) id = 'blockers';
  else if (q.includes('fix') || q.includes('first')) id = 'fix_first';
  else if (q.includes('ready') || q.includes('proceed')) id = 'ready';
  else if (q.includes('evidence') || q.includes('proof')) id = 'evidence';
  else if (q.includes('change') || q.includes('driver')) id = 'migration_changes';

  const ins = b.insights.find((i) => i.id === id) || b.insights[0];
  return { answer: ins.answer };
}

async function askCopilot(question) {
  appendCopilotMessage(question, '', true);

  if (state.selectedClusterVm && state.lastClusterBriefing) {
    try {
      const vm = state.selectedClusterVm;
      const data = await api(
        `/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}/copilot/ask`,
        {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ question, briefing: state.lastClusterBriefing }),
        },
      );
      appendCopilotMessage(question, data.data.answer || data.data.insight?.answer, false);
      return;
    } catch {
      /* fall through to local */
    }
  }

  if (state.lastJobId && state.selectedVm) {
    try {
      const data = await api(`/vms/${state.selectedVm.id}/copilot/ask`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ question, job_id: state.lastJobId }),
      });
      appendCopilotMessage(question, data.data.insight.answer, false);
      return;
    } catch {
      /* fall through to local */
    }
  }

  const { answer } = answerCopilotLocal(question);
  appendCopilotMessage(question, answer, false);
}

function tryParse(s) {
  try { return JSON.parse(s); } catch { return null; }
}

function escapeHtml(s) {
  const d = document.createElement('div');
  d.textContent = s;
  return d.innerHTML;
}

function showRaw(data) {
  const el = $('#rawJson');
  if (el) el.textContent = JSON.stringify(data, null, 2);
  window.GuestKitConsole?.appendEvidenceLog?.(JSON.stringify(data, null, 2));
}

/** @deprecated use showRaw — kept for cached older app.js callers */
function setRawJson(data) {
  showRaw(data);
}

function showYaml(yaml) {
  state.lastYaml = yaml;
  const yc = $('#yamlContent'); if (yc) yc.textContent = yaml || '';
  $('#yamlTab')?.classList.toggle('hidden', !yaml);
  $('#copyYamlBtn')?.classList.toggle('hidden', !yaml);
  $('#applyYamlBtn')?.classList.toggle('hidden', !yaml);
}

function setActiveTab(name) {
  $$('.tab').forEach((t) => t.classList.toggle('active', t.dataset.tab === name));
  $$('.tab-pane').forEach((p) => p.classList.remove('active'));
  const map = {
    summary: '#pane-summary',
    timeline: '#pane-timeline',
    risk: '#pane-risk',
    findings: '#pane-findings',
    diff: '#pane-diff',
    logs: '#pane-logs',
    copilot: '#pane-copilot',
    agent: '#pane-agent',
    yaml: '#pane-yaml',
    raw: '#pane-raw',
  };
  document.querySelector(map[name] || '#pane-raw')?.classList.add('active');
}

function onJobComplete(action, data) {
  const vm = state.selectedVm;
  if (!vm) return;

  const payload = extractPayload(data);
  const boot = payload?.bootability;
  const migrate = payload?.migration_score;
  const blockers = boot?.blockers || [];

  const patch = { lastOp: action, status: 'imported' };

  if (action === 'doctor' || action === 'agent-doctor') {
    patch.bootScore = boot?.score;
    patch.confidence = boot?.confidence;
    patch.blockers = blockers;
    patch.warnings = boot?.warnings;
    patch.checks = boot?.checks;
    patch.bootSummary = boot?.summary;
    patch.rootCause = payload?.root_cause;
    patch.status = blockers.length ? 'failed' : 'analyzed';
    if (payload?.copilot) {
      patch.briefing = payload.copilot;
      state.lastBriefing = payload.copilot;
    }
    if (action === 'agent-doctor') patch.agentOnline = true;
    markWizardComplete('assure');
    if (!state.wizardChain && !blockers.length) setWizardStep('plan');
  } else if (action === 'inspect') {
    patch.status = 'analyzed';
    patch.inspect = payload?.inspect || payload?.summary?.inspect
      || (payload?.os ? { os: payload.os, boot: payload.boot, kernel: payload.kernel } : null);
    markWizardComplete('assure');
    setActiveTab('summary');
  } else if (action === 'migration-plan') {
    patch.migrateScore = migrate?.score ?? boot?.score;
    patch.migrationPlan = payload;
    patch.status = 'ready';
    markWizardComplete('plan');
    if (!state.wizardChain) setWizardStep('launch');
  } else if (action === 'passport') {
    patch.passport = payload?.passport || payload;
    patch.migrateScore = payload?.passport?.scores?.migration ?? patch.migrateScore;
    patch.bootScore = payload?.passport?.scores?.boot ?? patch.bootScore;
    patch.status = payload?.passport?.hard_blocked ? 'failed' : 'ready';
    markWizardComplete('assure');
    try {
      const blob = new Blob([JSON.stringify(payload?.passport || payload, null, 2)], {
        type: 'application/json',
      });
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = `${(vm.name || 'vm').replace(/[^\w.-]+/g, '_')}-cutover-passport.json`;
      a.click();
      URL.revokeObjectURL(a.href);
      toast('Cutover Passport downloaded', 'ok');
    } catch (_) { /* ignore download errors */ }
  } else if (action === 'repair-plan') {
    patch.repairPlan = payload;
    if (payload?.before_score != null) patch.bootScore = payload.before_score;
    if (payload?.after_score != null) patch.bootScore = payload.after_score;
    patch.status = payload?.before_score != null ? 'analyzed' : patch.status;
  } else if (action === 'convert') {
    patch.lastOp = 'convert';
    patch.status = 'ready';
  } else if (action === 'provision') {
    markWizardComplete('launch');
    setWizardStep('launch');
  }

  updateVmCache(vm.id, patch);
  updateWizardFooter();

  if (payload?.copilot && (action === 'doctor' || action === 'migration-plan')) {
    setActiveTab('copilot');
    feed('Migration <strong>Ask Zeus</strong> briefing ready', 'ok');
  } else if (action === 'doctor') {
    const hasRisks = blockers.length || (patch.checks || []).some((c) => !c.passed);
    setActiveTab(hasRisks ? 'findings' : 'summary');
  } else if (action === 'inspect') {
    setActiveTab('summary');
  } else if (action === 'repair-plan' || action === 'migration-plan' || action === 'provision') {
    setActiveTab('summary');
  }
  window.GuestKitConsole?.scrollToEvidenceConsole?.();
  window.GuestKitConsole?.onJobCompleteConsole?.(action, vm.id);
  window.GuestKitConsole?.renderBrainPanel?.();
  window.GuestKitConsole?.renderEvidenceConsole?.(vm, getVmCache(vm.id));
  window.GuestKitNebula?.renderAllNebula?.(vm, getVmCache(vm.id));
  if (action === 'provision') window.GuestKitConsole?.showLaunchMonitor?.();
}

function pollJob(jobId, action) {
  if (state.pollTimer) clearInterval(state.pollTimer);

  return new Promise((resolve) => {
    const tick = async () => {
      try {
        const data = await api(`/jobs/${jobId}`);
        const live = data?.data?.live_status || {};
        const status = live.status || data?.data?.status || 'pending';

        // Renders must never block completion detection (legacy elements may be absent).
        try {
          showRaw(data);
          renderSummary(data, action);
          const js = $('#jobStatus'); if (js) js.textContent = status;
          const progress = live.progress ?? live.progress_percent;
          if (progress != null) setJobProgress(progress, live.message || status);
          else if (live.message) setJobProgress(null, live.message);
          window.GuestKitConsole?.appendEvidenceLog?.(`[${status}] ${live.message || action}${progress != null ? ` (${progress}%)` : ''}`);
          window.GuestKitConsole?.syncBrainJobTracker?.();
        } catch (e) { console.warn('[poll render]', e?.message || e); }

        if (status === 'completed') {
          clearInterval(state.pollTimer);
          try {
            hideJobTracker('completed');
            feed(`Job <span class="mono">${jobId.slice(0, 8)}…</span> completed`, 'ok');
            toast(`${action} finished`, 'ok');
            onJobComplete(action, data);
            if (action === 'provision' && data?.data?.yaml) {
              showYaml(data.data.yaml);
              setActiveTab('yaml');
            }
          } catch (e) { console.warn('[job complete]', e?.message || e); }
          resolve({ ok: true, data });
        } else if (status === 'failed') {
          clearInterval(state.pollTimer);
          hideJobTracker('failed');
          const rawErr = live.error || data?.data?.result?.error?.message || 'Job failed';
          const err = humanizeJobError(rawErr, state.selectedVm);
          feed(`Job failed: ${escapeHtml(err)}`, 'err');
          toast(err, 'err');
          state.lastFailedAction = action;
          $('#jobRetryBtn')?.classList.remove('hidden');
          if (state.selectedVm) {
            updateVmCache(state.selectedVm.id, {
              status: 'failed',
              lastOp: action,
              lastError: err,
            });
          }
          setScore(null, 'failed');
          resolve({ ok: false, error: err });
        }
      } catch {
        /* keep polling */
      }
    };

    tick();
    state.pollTimer = setInterval(tick, 2500);
  });
}

function appendAgentMessage(text, role = 'assistant') {
  const chat = $('#agentChat');
  const ph = $('#agentPlaceholder');
  ph?.classList.add('hidden');
  const el = document.createElement('div');
  el.className = `agent-msg ${role}`;
  el.innerHTML = role === 'assistant' ? escapeHtml(text).replace(/\n/g, '<br>') : escapeHtml(text);
  chat.appendChild(el);
  chat.scrollTop = chat.scrollHeight;
}

function renderAgentChips() {
  const wrap = $('#agentChips');
  if (!wrap) return;
  wrap.innerHTML = '';
  AGENT_QUICK_RPC.forEach((item) => {
    const chip = document.createElement('button');
    chip.type = 'button';
    chip.className = 'agent-chip';
    chip.textContent = item.label;
    chip.addEventListener('click', () => agentTalk(item.method, item.params, item.label));
    wrap.appendChild(chip);
  });
}

async function pingAgent() {
  const vm = state.selectedVm;
  if (!vm) {
    toast('Select a VM first', 'err');
    return;
  }
  const proxyUrl = getAgentProxyUrl();
  if (!proxyUrl) {
    toast('Enter agent proxy URL', 'err');
    return;
  }
  setAgentStatus('pinging…');
  try {
    const data = await api(`/vms/${vm.id}/agent/ping`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ proxy_url: proxyUrl }),
    });
    const r = data.data;
    state.agentReachable = r.reachable;
    state.agentProxyUrl = proxyUrl;
    localStorage.setItem('zyvor.agentProxyUrl', proxyUrl);
    setAgentStatus(r.reachable ? 'agent online' : 'agent unreachable', r.reachable);
    appendAgentMessage(r.reachable ? 'Agent reachable via proxy.' : `Unreachable: ${r.error || 'unknown'}`, 'system');
    if (r.agent) appendAgentMessage(JSON.stringify(r.agent, null, 2));
    toast(r.reachable ? 'Agent online' : 'Agent unreachable', r.reachable ? 'ok' : 'err');
  } catch (e) {
    state.agentReachable = false;
    setAgentStatus('ping failed', false);
    toast(e.message, 'err');
  }
}

async function runAgentAction(kind) {
  const vm = state.selectedVm;
  if (!vm) {
    toast('Select a VM first', 'err');
    return { ok: false };
  }
  const proxyUrl = getAgentProxyUrl();
  if (!proxyUrl) {
    toast('Enter agent proxy URL', 'err');
    return { ok: false };
  }

  const target = getTarget();
  let path;
  let body = { proxy_url: proxyUrl, target };

  if (kind === 'evidence') path = `/vms/${vm.id}/agent/evidence`;
  else if (kind === 'doctor') path = `/vms/${vm.id}/agent/doctor?target=${encodeURIComponent(target)}`;
  else if (kind === 'capabilities') {
    return agentTalk('guestkit.getCapabilities', {}, 'Capabilities');
  } else if (kind === 'metrics') {
    return agentTalk('guestkit.getMetrics', {}, 'Metrics');
  } else {
    return { ok: false };
  }

  feed(`Enqueueing <strong>agent ${escapeHtml(kind)}</strong>…`);
  setActiveTab('summary');
  try {
    const data = await api(path, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    const jobId = data?.data?.job_id;
    if (!jobId) return { ok: false };
    state.lastJobId = jobId;
    const action = kind === 'doctor' ? 'agent-doctor' : `agent-${kind}`;
    showJobTracker(action, jobId);
    return await pollJob(jobId, action);
  } catch (e) {
    toast(e.message, 'err');
    return { ok: false, error: e.message };
  }
}

async function agentTalk(method, params = {}, label = method) {
  const vm = state.selectedVm;
  if (!vm) {
    toast('Select a VM first', 'err');
    return { ok: false };
  }
  const proxyUrl = getAgentProxyUrl();
  if (!proxyUrl) {
    toast('Enter agent proxy URL', 'err');
    return { ok: false };
  }

  appendAgentMessage(label || method, 'user');
  setActiveTab('agent');

  try {
    const data = await api(`/vms/${vm.id}/agent/rpc`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ proxy_url: proxyUrl, method, params }),
    });
    const jobId = data?.data?.job_id;
    if (!jobId) return { ok: false };
    state.lastJobId = jobId;
    showJobTracker('agent-rpc', jobId);
    const result = await pollJob(jobId, 'agent-rpc');
    if (result.ok) {
      const payload = extractPayload(result.data);
      const text = typeof payload === 'object'
        ? JSON.stringify(payload, null, 2)
        : String(payload);
      appendAgentMessage(text);
      showRaw(result.data);
    }
    return result;
  } catch (e) {
    appendAgentMessage(e.message, 'system');
    toast(e.message, 'err');
    return { ok: false };
  }
}

async function runClusterGuestkitInspect() {
  const vm = state.selectedClusterVm;
  if (!vm) {
    toast('Select a cluster VM first', 'err');
    return { ok: false };
  }
  feed(`GuestKit inspect on <strong>${escapeHtml(vm.namespace)}/${escapeHtml(vm.name)}</strong>…`);
  setActiveTab('summary');
  setWizardStep('assure');
  const inspectPh = $('#summaryPlaceholder');
  if (inspectPh) {
    inspectPh.textContent = 'Running GuestKit inspect…';
    inspectPh.classList.remove('hidden');
  }
  $('#summaryContent')?.classList.add('hidden');

  const running = String(vm.phase || vm.status || '').toLowerCase().includes('run');
  if (running) {
    await fetchClusterGuestInfo(false);
    if (state.lastClusterGuestInfo?.agent_connected) {
      toast('Live guest agent connected — showing runtime guest info', 'ok');
      return { ok: true };
    }
    toast('Stop the VM for offline GuestKit disk inspect', 'err');
    await fetchClusterBootInspect(false);
    return { ok: false };
  }

  try {
    const data = await api(
      `/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}/inspect`,
      { method: 'POST' },
    );
    const jobId = data?.data?.job_id;
    if (!jobId) {
      toast('Inspect job did not start', 'err');
      return { ok: false };
    }
    state.lastJobId = jobId;
    showJobTracker('inspect', jobId);
    const result = await pollJob(jobId, 'inspect');
    if (result.ok) {
      const payload = extractPayload(result.data);
      state.lastClusterInspect = payload?.inspect || null;
      if (state.lastClusterGuestInfo) {
        renderClusterGuestSummary(state.lastClusterGuestInfo, state.lastClusterBootInspect);
      }
    }
    return result;
  } catch (e) {
    toast(e.message, 'err');
    feed(`Inspect failed: ${escapeHtml(e.message)}`, 'err');
    return { ok: false, error: e.message };
  }
}

async function runClusterGuestkitJob(action) {
  const vm = state.selectedClusterVm;
  if (!vm) {
    toast('Select a cluster VM first', 'err');
    return { ok: false };
  }
  if (!clusterVmStopped(vm)) {
    toast('Stop the VM for offline GuestKit disk workflows', 'err');
    return { ok: false };
  }
  const target = getTarget();
  const base = `/kubevirt/vms/${encodeURIComponent(vm.namespace)}/${encodeURIComponent(vm.name)}`;
  let path = `${base}/${action}`;
  if (action === 'doctor' || action === 'migration-plan') {
    path += `?target=${encodeURIComponent(target)}&explain=true`;
  }
  feed(`GuestKit <strong>${escapeHtml(action)}</strong> on <strong>${escapeHtml(vm.namespace)}/${escapeHtml(vm.name)}</strong>…`);
  setActiveTab('summary');
  setWizardStep(action === 'provision' ? 'launch' : 'plan');
  try {
    const data = await api(path, { method: 'POST' });
    if (action === 'provision' && data?.data?.yaml) {
      renderSummary({ data: data.data }, action);
      showYaml(data.data.yaml);
      setActiveTab('yaml');
      toast('KubeVirt YAML ready', 'ok');
      return { ok: true, data };
    }
    const jobId = data?.data?.job_id;
    if (!jobId) {
      toast(`${action} did not start`, 'err');
      return { ok: false };
    }
    state.lastJobId = jobId;
    showJobTracker(action, jobId);
    return await pollJob(jobId, action);
  } catch (e) {
    toast(e.message, 'err');
    return { ok: false, error: e.message };
  }
}

async function runClusterProvision() {
  return runClusterGuestkitJob('provision');
}

async function runClusterGuestkitDoctor() {
  return runClusterGuestkitJob('doctor');
}

async function runClusterAction(action) {
  if (action === 'inspect') return runClusterGuestkitInspect();
  if (action === 'doctor') {
    const vm = state.selectedClusterVm;
    const running = String(vm?.phase || vm?.status || '').toLowerCase().includes('run');
    if (running && state.lastClusterGuestInfo?.agent_connected) {
      await fetchClusterGuestInfo(true);
      return { ok: true };
    }
    if (!running) return runClusterGuestkitDoctor();
    toast('Stop VM for offline doctor, or install guest agent for live checks', 'err');
    await fetchClusterBootInspect(true);
    return { ok: false };
  }
  if (action === 'migration-plan' || action === 'repair-plan' || action === 'provision') {
    return runClusterGuestkitJob(action);
  }
  toast('Unsupported cluster workflow', 'err');
  return { ok: false };
}

async function runAction(action) {
  if (action === 'provision') {
    return window.GuestKitConsole?.showLaunchPreview?.(() => runActionInner('provision')) || runActionInner('provision');
  }
  return runActionInner(action);
}

// Full analysis: inspect -> doctor -> migration-plan, merged into one report.
async function runFullAnalysis() {
  const vm = state.selectedVm;
  if (!vm) { toast('Select a disk from the vault first', 'err'); return { ok: false }; }
  if (isOnlineMode()) setInspectionMode('offline'); // analysis runs on the offline disk
  const steps = [
    ['inspect', 'Fingerprinting OS'],
    ['doctor', 'Scoring bootability'],
    ['migration-plan', 'Planning migration'],
  ];
  const reRender = () => window.GuestKitNebula?.renderAllNebula?.(vm, getVmCache(vm.id));
  const emit = (phase, detail) => { try { window.dispatchEvent(new CustomEvent('gk:analyze', { detail: Object.assign({ phase, vm: vm.name || vm.id }, detail) })); } catch (e) {} };
  feed('Ask Zeus — running full analysis…');
  emit('start', { steps: steps.map(s => s[1]), n: steps.length });
  let ok = true;
  try {
    for (let i = 0; i < steps.length; i++) {
      const [step, label] = steps[i];
      state.analyzing = { label, i: i + 1, n: steps.length };
      emit('step', { label, i: i + 1, n: steps.length });
      reRender();                       // show the progress banner immediately
      const r = await runActionInner(step);   // onJobComplete fills the report on completion
      if (r && r.ok === false) {
        ok = false;
        feed(`Analysis stopped at ${escapeHtml(label)}: ${escapeHtml(r.error || 'failed')}`, 'err');
        emit('error', { label, error: r.error || 'failed' });
        break;
      }
    }
  } finally {
    state.analyzing = null;
    reRender();
  }
  feed('Ask Zeus — intelligence report ready', 'ok');
  emit('done', { ok, score: getVmCache(vm.id)?.bootScore });
  return { ok: true };
}
window.runFullAnalysis = runFullAnalysis;

async function runActionInner(action) {
  if (state.selectedClusterVm && !state.selectedVm) {
    return runClusterAction(action);
  }
  const vm = state.selectedVm;
  if (!vm) {
    toast('Select a VM from the fleet first', 'err');
    return { ok: false };
  }

  if (isOnlineMode()) {
    if (action === 'inspect') return runAgentAction('evidence');
    if (action === 'doctor') return runAgentAction('doctor');
    toast('Use offline mode for migrate/repair/provision disk workflows', 'err');
    return { ok: false };
  }

  if (isSmokeDisk(vm) && action !== 'provision') {
    const alt = pickBestVm(state.vms.filter((v) => v.id !== vm.id));
    toast('Smoke disk has no OS — select a real cloud image in Fleet', 'err');
    feed('Skipped workflow on placeholder disk — pick Cirros or Ubuntu minimal', 'err');
    if (alt) {
      selectVm(alt);
      toast(`Switched to ${alt.name}`, 'ok');
    }
    return { ok: false };
  }

  let path = `/vms/${vm.id}/${action}`;
  const target = getTarget();
  if (action === 'doctor' || action === 'migration-plan' || action === 'passport') {
    path += `?target=${encodeURIComponent(target)}&explain=true`;
  }

  feed(`Enqueueing <strong>${escapeHtml(action)}</strong>…`);

  const stepMap = { provision: 'launch', 'migration-plan': 'plan', 'repair-plan': 'plan', passport: 'assure', doctor: 'assure', inspect: 'assure' };
  setWizardStep(stepMap[action] || 'assure');

  try {
    const data = await api(path, { method: 'POST' });
    showRaw(data);
    const jobPh = $('#summaryPlaceholder');
    if (jobPh) {
      jobPh.textContent = 'Job running — results will stream in…';
      jobPh.classList.remove('hidden');
    }
    $('#summaryContent')?.classList.add('hidden');
    setScore(null);
    state.lastFailedAction = null;
    $('#jobRetryBtn').classList.add('hidden');

    if (action !== 'provision') showYaml(null);

    if (action === 'provision' && data?.data?.yaml) {
      renderSummary({ data: data.data }, action);
      showYaml(data.data.yaml);
      setActiveTab('yaml');
      toast('KubeVirt YAML ready', 'ok');
      feed('Provision YAML generated (sync)', 'ok');
      onJobComplete(action, { data: data.data });
      return { ok: true, data };
    }

    const jobId = data?.data?.job_id;
    if (jobId) {
      state.lastJobId = jobId;
      window.GuestKitConsole?.clearEvidenceLogs?.();
      window.GuestKitConsole?.appendEvidenceLog?.(`[queued] ${action} job ${jobId.slice(0, 8)}…`);
      showJobTracker(action, jobId);
      toast('Job queued', 'ok');
      return await pollJob(jobId, action);
    }
    return { ok: true, data };
  } catch (e) {
    toast(e.message, 'err');
    feed(`${escapeHtml(action)} failed: ${escapeHtml(e.message)}`, 'err');
    return { ok: false, error: e.message };
  }
}

async function runWizardChain() {
  if (!state.selectedVm) {
    toast('Select a VM from the fleet first', 'err');
    return;
  }
  state.wizardChain = true;
  feed('Starting <strong>full migration</strong> chain…', '');

  try {
    const doctor = await runAction('doctor');
    if (!doctor.ok) return;

    const cache = getVmCache(state.selectedVm.id);
    if (cache.blockers?.length) {
      toast('Migration blocked — resolve blockers first', 'err');
      return;
    }

    const plan = await runAction('migration-plan');
    if (!plan.ok) return;

    setWizardStep('launch');
    toast('Plan complete — ready to provision', 'ok');
    feed('Migration chain complete — generate YAML when ready', 'ok');
  } finally {
    state.wizardChain = false;
  }
}

function setupInspectionMode() {
  if ($('#agentProxyUrl') && state.agentProxyUrl) {
    $('#agentProxyUrl').value = state.agentProxyUrl;
  }
  setInspectionMode(state.inspectionMode);
  $('#modeOfflineBtn')?.addEventListener('click', () => setInspectionMode('offline'));
  $('#modeOnlineBtn')?.addEventListener('click', () => setInspectionMode('online'));
  $('#agentPingBtn')?.addEventListener('click', () => pingAgent());
  $('#agentProxyUrl')?.addEventListener('change', (e) => {
    state.agentProxyUrl = e.target.value.trim();
    localStorage.setItem('zyvor.agentProxyUrl', state.agentProxyUrl);
  });
  $$('[data-agent-action]').forEach((btn) => {
    btn.addEventListener('click', () => {
      if (!btn.disabled) runAgentAction(btn.dataset.agentAction);
    });
  });
  $('#agentForm')?.addEventListener('submit', (e) => {
    e.preventDefault();
    const input = $('#agentInput');
    const raw = input.value.trim();
    if (!raw) return;
    input.value = '';
    if (raw.startsWith('guestkit.') || raw.startsWith('guest-')) {
      agentTalk(raw, {});
    } else if (raw.startsWith('{')) {
      try {
        const body = JSON.parse(raw);
        agentTalk(body.method || 'guestkit.exec', body.params || {});
      } catch {
        appendAgentMessage('Invalid JSON — use {"method":"guestkit.ping","params":{}}', 'system');
      }
    } else {
      agentTalk('guestkit.exec', { command: raw.split(/\s+/) }, raw);
    }
  });
  renderAgentChips();
}

function setupCopilot() {
  const form = $('#copilotForm');
  if (form) {
    form.addEventListener('submit', (e) => {
      e.preventDefault();
      const input = $('#copilotInput');
      const q = input ? input.value.trim() : '';
      if (!q) return;
      input.value = '';
      setActiveTab('copilot');
      askCopilot(q);
    });
  }

  const banner = $('#copilotBannerAction');
  if (banner) {
    banner.addEventListener('click', (e) => {
      const action = e.currentTarget.dataset.action;
      if (action) runWorkflow(action);
    });
  }
}

function setupWizard() {
  renderWizardBar();
  updateWizardFooter();

  $('#wizardBackBtn').addEventListener('click', () => {
    const idx = WIZARD_STEPS.findIndex((s) => s.id === state.wizard.step);
    if (idx > 0) setWizardStep(WIZARD_STEPS[idx - 1].id);
  });

  $('#wizardContinueBtn').addEventListener('click', () => {
    const idx = WIZARD_STEPS.findIndex((s) => s.id === state.wizard.step);
    if (idx < WIZARD_STEPS.length - 1 && state.wizard.completed.has(state.wizard.step)) {
      setWizardStep(WIZARD_STEPS[idx + 1].id);
    }
  });

  $('#wizardActionBtn').addEventListener('click', () => {
    const action = $('#wizardActionBtn').dataset.action;
    if (action) runAction(action);
  });

  $('#wizardChainBtn').addEventListener('click', () => runWizardChain());

  $('#jobRetryBtn').addEventListener('click', () => {
    if (state.lastFailedAction) runAction(state.lastFailedAction);
  });
}

function setupTheme() {
  // Zyvor orange/white GA is the sole product UX — no multi-theme skins.
  try { localStorage.removeItem('zyvor.theme'); } catch (e) {}
  delete document.documentElement.dataset.theme;
}

function setupGlassToggle() {
  setupTheme();
}

function setupActions() {
  const bindAction = (el) => {
    if (!el.dataset.action) return;
    el.addEventListener('click', () => {
      if (el.disabled) return;
      runAction(el.dataset.action);
    });
  };

  $$('.action-card').forEach(bindAction);
  $$('.dock-item[data-action]').forEach(bindAction);

  $$('[data-goto]').forEach((btn) => {
    btn.addEventListener('click', () => scrollToPanel(btn.dataset.goto));
  });

  $$('.pipe-step').forEach((btn) => {
    btn.addEventListener('click', () => {
      const step = btn.dataset.step;
      if (!step) return;
      if (step === 'cluster') {
        openClusterFleet();
        return;
      }
      if (!canReachStep(step)) {
        toast('Complete earlier pipeline steps to unlock actions — showing preview');
      }
      scrollToPanel(step);
    });
  });

  $('#openClusterFleetBtn')?.addEventListener('click', () => openClusterFleet());
  $('#openClusterFleetBtn2')?.addEventListener('click', () => openClusterFleet());
  $('#ingestUploadBtn')?.addEventListener('click', () => triggerDiskUpload());
  $('#ingestUploadBtn2')?.addEventListener('click', () => triggerDiskUpload());
  $('#openClusterFleetBtn2')?.addEventListener('click', () => openClusterFleet());
  $('#serverStorageUploadBtn2')?.addEventListener('click', () => {
    scrollToPanel('ingest');
    document.getElementById('serverStorageBrowser')?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  });
  $('#menubarClusterBtn')?.addEventListener('click', () => openClusterFleet());

  $('#fleetUploadBtn')?.addEventListener('click', () => triggerDiskUpload());
  $('#fleetBrowseClusterBtn')?.addEventListener('click', () => openClusterFleet());
  $('#fleetRefreshClusterBtn')?.addEventListener('click', () => loadClusterFleet());
  $('#vmtoolsAutoInstall')?.addEventListener('change', () => saveVmtoolsPolicy());
  $('#vmtoolsAutoUpgrade')?.addEventListener('change', () => saveVmtoolsPolicy());
  $('#vmtoolsReconcileBtn')?.addEventListener('click', () => reconcileVmtoolsFleet());
  $('#packetwolfSyncBtn')?.addEventListener('click', () => syncPacketwolfFleet());
  $('#fleetEmptyBrowseCluster')?.addEventListener('click', () => openClusterFleet());
  $('#fleetEmptyImport')?.addEventListener('click', () => triggerDiskUpload());

  $$('.tab').forEach((tab) => {
    tab.addEventListener('click', () => setActiveTab(tab.dataset.tab));
  });

  $('#refreshFleetBtn').addEventListener('click', () => {
    if (state.fleetMode === 'cluster') loadClusterFleet();
    else loadFleet();
    checkHealth();
    toast('Fleet refreshed', 'ok');
  });

  $$('.fleet-tab').forEach((tab) => {
    tab.addEventListener('click', () => {
      const mode = tab.dataset.fleet;
      if (!mode || mode === state.fleetMode) return;
      setFleetMode(mode);
      if (mode === 'cluster') loadClusterFleet();
      else loadFleet();
    });
  });

  $('#fleetEmptyRefresh')?.addEventListener('click', () => loadClusterFleet());

  document.addEventListener('click', (e) => {
    const approveBtn = e.target.closest('[data-guest-approve]');
    if (approveBtn) {
      approveGuestAction(approveBtn.dataset.guestApprove);
      return;
    }
    const rejectBtn = e.target.closest('[data-guest-reject]');
    if (rejectBtn) {
      rejectGuestAction(rejectBtn.dataset.guestReject);
      return;
    }
    const btn = e.target.closest('[data-guest-action]');
    if (!btn || !state.selectedClusterVm) return;
    const action = btn.dataset.guestAction;
    if (action === 'restart-unit') clusterGuestRestartUnit(btn.dataset.unit);
    else if (action === 'view-journal') clusterGuestViewJournal(btn.dataset.unit);
    else if (action === 'collect-bundle') clusterGuestCollectBundle();
    else if (action === 'refresh') fetchClusterGuestInfo(true);
  });

  $$('[data-cluster-action]').forEach((btn) => {
    btn.addEventListener('click', () => {
      const vm = state.selectedClusterVm;
      if (!vm) {
        toast('Select a cluster VM first', 'err');
        return;
      }
      if (btn.dataset.clusterAction === 'guest-info') fetchClusterGuestInfo(true);
      else if (btn.dataset.clusterAction === 'guestkit-inspect') runClusterGuestkitInspect();
      else if (btn.dataset.clusterAction === 'boot-inspect') fetchClusterBootInspect(true);
      else if (btn.dataset.clusterAction === 'install-agent') installClusterGuestAgent('auto');
      else if (btn.dataset.clusterAction === 'install-iso') installClusterGuestAgent('iso');
      else if (btn.dataset.clusterAction === 'vmtools-diagnostics') runVmToolsDiagnostics();
      else if (btn.dataset.clusterAction === 'vmtools-quiesce') clusterVmtoolsOp('quiesce');
      else if (btn.dataset.clusterAction === 'vmtools-unquiesce') clusterVmtoolsOp('unquiesce');
      else if (btn.dataset.clusterAction === 'vmtools-reboot') clusterVmtoolsOp('reboot');
      else if (btn.dataset.clusterAction === 'vmtools-shutdown') clusterVmtoolsOp('shutdown');
      else if (btn.dataset.clusterAction === 'zeus-vm') {
        window.open(zeusVmUrl(vm.namespace, vm.name, 'guest'), '_blank', 'noopener,noreferrer');
      } else if (btn.dataset.clusterAction === 'export-disk') {
        exportClusterDisk(false);
      } else if (btn.dataset.clusterAction === 'start' || btn.dataset.clusterAction === 'stop' || btn.dataset.clusterAction === 'restart') {
        clusterLifecycle(btn.dataset.clusterAction);
      }
    });
  });

  $('#copyYamlBtn').addEventListener('click', async () => {
    if (!state.lastYaml) return;
    try {
      await navigator.clipboard.writeText(state.lastYaml);
      toast('YAML copied', 'ok');
    } catch {
      toast('Copy failed', 'err');
    }
  });

  $('#applyYamlBtn')?.addEventListener('click', () => applyYamlToCluster());

  $('#fleetNamespaceFilter')?.addEventListener('change', (e) => {
    state.fleetFilters.namespace = e.target.value;
    if (state.fleetMode === 'cluster') loadClusterFleet();
  });

  let fleetSearchTimer;
  $('#fleetSearchFilter')?.addEventListener('input', (e) => {
    clearTimeout(fleetSearchTimer);
    fleetSearchTimer = setTimeout(() => {
      state.fleetFilters.search = e.target.value.trim();
      if (state.fleetMode === 'cluster') loadClusterFleet();
    }, 300);
  });

  $$('.phase-chip').forEach((chip) => {
    chip.addEventListener('click', () => {
      $$('.phase-chip').forEach((c) => c.classList.toggle('active', c === chip));
      state.fleetFilters.phase = chip.dataset.phase || '';
      if (state.fleetMode === 'cluster') loadClusterFleet();
    });
  });

  $('#clusterDrawerClose')?.addEventListener('click', () => {
    $('#clusterDetailDrawer')?.classList.add('hidden');
  });
}

async function applyUrlContext() {
  const params = new URLSearchParams(window.location.search);
  const pathMatch = window.location.pathname.match(/\/embed\/vm\/([^/]+)\/([^/]+)/);
  const namespace = params.get('namespace') || (pathMatch ? decodeURIComponent(pathMatch[1]) : null);
  const vmName = params.get('vm') || (pathMatch ? decodeURIComponent(pathMatch[2]) : null);
  const action = params.get('action');

  if (namespace && vmName) {
    setFleetMode('cluster');
    await loadClusterFleet();
    const vm = state.clusterVms.find((v) => v.namespace === namespace && v.name === vmName);
    if (vm) {
      selectClusterVm(vm);
      if (action === 'inspect') await runClusterGuestkitInspect();
      else if (action === 'doctor') await runClusterAction('doctor');
      else await fetchClusterGuestInfo(false);
    } else {
      toast(`VM ${namespace}/${vmName} not found in cluster`, 'err');
    }
  }
}

async function init() {
  try {
  window.GuestKitAuth?.captureTokenFromUrl?.();
  const me = await window.GuestKitAuth?.requireAuthOrRedirect?.();
  if (me === null) return;
  window.GuestKitAuth?.initSettingsModal?.();
  window.GuestKitAuth?.initUserMenu?.();

  window.state = state;
  window.api = api;
  window.getVmCache = getVmCache;
  window.renderFleet = renderFleet;
  window.updateVmCache = updateVmCache;
  window.escapeHtml = escapeHtml;
  window.fmtBytes = fmtBytes;
  window.isSmokeDisk = isSmokeDisk;
  window.isFleetDisk = isFleetDisk;
  window.pickBestVm = pickBestVm;
  window.vmStatusLabel = vmStatusLabel;
  window.selectVm = selectVm;
  window.runAction = runAction;
  window.pollJob = pollJob;
  window.showJobTracker = showJobTracker;
  window.loadFleet = loadFleet;
  window.uploadFile = uploadFile;
  window.scrollToPanel = scrollToPanel;
  window.setActiveTab = setActiveTab;
  window.updateSelectionPanels = updateSelectionPanels;
  window.toast = toast;
  window.extractPayload = extractPayload;
  window.API_BASE = API_BASE;
  window.fetchClusterCopilotBriefing = fetchClusterCopilotBriefing;
  window.answerCopilotLocal = answerCopilotLocal;
  window.renderCopilot = renderCopilot;

  // Legacy setup steps reference elements that the newer nebula console owns.
  // Run each in isolation so a missing element can never abort the whole app.
  const safe = (fn) => { try { return fn(); } catch (e) { console.warn('[init] skipped:', e?.message || e); } };
  safe(setupDropzone);
  safe(setupGlassToggle);
  safe(() => window.GuestKitConsole?.initGuestKitConsole?.());
  safe(() => window.GuestKitFeatures?.initGuestKitFeatures?.());
  safe(() => window.GuestKitNebula?.initGuestKitNebula?.());
  safe(setupWizard);
  safe(setupInspectionMode);
  safe(setupCopilot);
  safe(setupActions);
  safe(setupServerStorage);
  safe(updateCopilotPlaceholder);
  safe(() => setDockEnabled(false));
  await loadUiConfig();
  await loadServerStorageRoots();
  await browseServerStorage('', state.serverStorage.rootId);
  setFleetMode(state.fleetMode);
  setWizardStep('ingest');
  updateFleetToolbar();
  await checkHealth();
  if (state.fleetMode === 'cluster') await loadClusterFleet();
  else await loadFleet();
  await applyUrlContext();
  setInterval(checkHealth, 30000);
  window.GuestKitConsole?.refreshHudStatus?.();
  feed(state.fleetMode === 'cluster' ? 'KubeVirt cluster fleet ready' : 'Ready — ingest a disk or switch to KubeVirt cluster', 'ok');
  } catch (e) {
    console.error('GuestKit init failed:', e);
    setHealth(false);
    toast(`UI init failed: ${e.message}`, 'err');
  }
}

init();
