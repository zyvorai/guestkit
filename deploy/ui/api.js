// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

/** GuestKit API client — wires Apple shell to zyvor-api /api/v1 */

const API_BASE = window.ZYVOR_API_URL || '/api/v1';
const AUTH_TOKEN_KEY = 'guestkit.authToken';
const AUTH_USER_KEY = 'guestkit.authUser';

function getAuthToken() {
  return localStorage.getItem(AUTH_TOKEN_KEY) || '';
}
function setAuthToken(token) {
  if (token) localStorage.setItem(AUTH_TOKEN_KEY, token);
  else localStorage.removeItem(AUTH_TOKEN_KEY);
}
function clearAuth() {
  localStorage.removeItem(AUTH_TOKEN_KEY);
  localStorage.removeItem(AUTH_USER_KEY);
}
function authHeaders(extra = {}) {
  const h = { ...extra };
  const t = getAuthToken();
  if (t) h.Authorization = `Bearer ${t}`;
  return h;
}

async function api(path, options = {}) {
  const headers = authHeaders(options.headers || {});
  const res = await fetch(`${API_BASE}${path}`, { ...options, headers });
  const data = await res.json().catch(() => ({}));
  if (res.status === 401) {
    clearAuth();
    throw new Error('Authentication required');
  }
  if (!res.ok) throw new Error(data.message || data.error || data.detail || res.statusText);
  return data;
}

async function fetchAuthConfig() {
  try {
    const res = await fetch(`${API_BASE}/auth/config`);
    if (!res.ok) return { auth_enabled: false, allow_local_bypass: true };
    const data = await res.json();
    return data.data || { auth_enabled: false, allow_local_bypass: true };
  } catch {
    return { auth_enabled: false, allow_local_bypass: true };
  }
}

async function localLogin(username, password) {
  const body = username
    ? JSON.stringify({ username, password })
    : undefined;
  const res = await fetch(`${API_BASE}/auth/local`, {
    method: 'POST',
    headers: authHeaders(body ? { 'Content-Type': 'application/json' } : {}),
    body,
  });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data.message || 'Local login failed');
  if (data.data?.token) setAuthToken(data.data.token);
  if (data.data?.user) localStorage.setItem(AUTH_USER_KEY, JSON.stringify(data.data.user));
  return data.data;
}

async function checkHealth() {
  const res = await fetch(`${API_BASE}/health`, { headers: authHeaders() });
  if (!res.ok) throw new Error(`health ${res.status}`);
  return res.json().catch(() => ({ ok: true }));
}

async function listVms() {
  const data = await api('/vms');
  return data.data || [];
}

async function importVm(file, onProgress) {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    const form = new FormData();
    form.append('file', file);
    xhr.upload.addEventListener('progress', (e) => {
      if (e.lengthComputable && onProgress) onProgress(Math.round((e.loaded / e.total) * 100));
    });
    xhr.addEventListener('load', () => {
      let data = {};
      try { data = JSON.parse(xhr.responseText); } catch { /* */ }
      if (xhr.status >= 200 && xhr.status < 300) resolve(data.data || data);
      else reject(new Error(data.message || data.error || xhr.statusText));
    });
    xhr.addEventListener('error', () => reject(new Error('Upload failed')));
    xhr.open('POST', `${API_BASE}/vms/import`);
    const token = getAuthToken();
    if (token) xhr.setRequestHeader('Authorization', `Bearer ${token}`);
    xhr.send(form);
  });
}

async function importFromUrl(url) {
  const data = await api('/vms/import-from-url', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ url }),
  });
  return data.data;
}

function pollJob(jobId, { intervalMs = 2000, timeoutMs = 300000, onTick } = {}) {
  const started = Date.now();
  return new Promise((resolve, reject) => {
    const tick = async () => {
      try {
        if (Date.now() - started > timeoutMs) {
          reject(new Error('Job timed out'));
          return;
        }
        const data = await api(`/jobs/${jobId}`);
        const live = data?.data?.live_status || {};
        const status = live.status || data?.data?.status || 'pending';
        if (onTick) onTick({ status, live, data });
        if (status === 'completed') {
          resolve({ ok: true, data: data.data, result: data.data?.result || live.result });
          return;
        }
        if (status === 'failed') {
          const err = live.error || data?.data?.result?.error?.message || 'Job failed';
          reject(new Error(err));
          return;
        }
      } catch (e) {
        if (e.message === 'Authentication required') {
          reject(e);
          return;
        }
      }
      setTimeout(tick, intervalMs);
    };
    tick();
  });
}

async function enqueueAndPoll(path, action, opts) {
  const data = await api(path, { method: 'POST' });
  const jobId = data.data?.job_id || data.data?.id;
  if (!jobId) throw new Error(`No job id from ${action}`);
  return pollJob(jobId, opts);
}

function doctorPath(vmId, target, explain = true) {
  const q = new URLSearchParams({ target: target || 'kvm', explain: String(!!explain) });
  return `/vms/${vmId}/doctor?${q}`;
}
function planPath(vmId, target) {
  const q = new URLSearchParams({ target: target || 'kvm', explain: 'true' });
  return `/vms/${vmId}/migration-plan?${q}`;
}
function passportPath(vmId, target) {
  const q = new URLSearchParams({ target: target || 'kvm' });
  return `/vms/${vmId}/passport?${q}`;
}
function provisionPath(vmId, apply = false) {
  return `/vms/${vmId}/provision?apply=${apply ? 'true' : 'false'}`;
}

function extractDoctorView(result) {
  // Worker wraps doctor JSON under result.data (sometimes result.result / boot_report).
  const payload =
    result?.data?.bootability ? result.data :
    result?.result?.data?.bootability ? result.result.data :
    result?.result?.bootability ? result.result :
    result?.bootability ? result :
    result?.result || result || {};
  const boot = payload.bootability || payload.boot_report || {};
  const score = Number(boot.score ?? payload.score ?? payload.boot_score);
  const warnings = boot.warnings || [];
  const blockers = boot.blockers || [];
  const checks = boot.checks || [];
  const findings = payload.findings || [
    ...blockers.map((b) => ({
      severity: 'high',
      title: b.title || b.check_id || 'Blocker',
      detail: b.message || '',
      fix: b.remediation || '',
    })),
    ...warnings.map((w) => ({
      severity: 'medium',
      title: w.title || w.check_id || 'Warning',
      detail: w.message || '',
      fix: w.remediation || '',
    })),
    ...checks.filter((c) => c.passed === false).map((c) => ({
      severity: (c.severity || 'medium').toLowerCase(),
      title: c.name || c.id || 'Check',
      detail: c.message || '',
      fix: c.remediation || '',
    })),
  ];
  const normalized = (Array.isArray(findings) ? findings : []).map((f) => ({
    severity: f.severity || f.level || 'info',
    title: f.title || f.id || f.check || 'Finding',
    detail: f.detail || f.message || f.evidence || f.summary || '',
    fix: f.fix || f.recommendation || f.remediation || '',
  }));
  let decision = payload.decision || payload.verdict || boot.verdict || '';
  if (!decision && !Number.isNaN(score)) {
    decision = score >= 90 ? 'SAFE — ready to cut over' : score >= 70 ? 'REVIEW — fix blockers first' : 'STOP — do not power on';
  }
  return {
    score: Number.isNaN(score) ? null : Math.round(score),
    decision,
    findings: normalized,
    raw: payload,
  };
}

window.GuestKitAPI = {
  API_BASE,
  getAuthToken,
  setAuthToken,
  clearAuth,
  authHeaders,
  api,
  fetchAuthConfig,
  localLogin,
  checkHealth,
  listVms,
  importVm,
  importFromUrl,
  pollJob,
  enqueueAndPoll,
  doctorPath,
  planPath,
  passportPath,
  provisionPath,
  extractDoctorView,
};
