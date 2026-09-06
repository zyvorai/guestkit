/* SPDX-License-Identifier: Apache-2.0 */
/* GuestKit Zyvor orange/white UX controller. No dependencies, no backend contract changes. */
(() => {
  'use strict';

  const isLogin = Boolean(document.querySelector('.login-page'));
  document.documentElement.classList.add('zyvor-ux');
  document.body?.classList.add('zyvor-ux');

  applyZyvorBranding();

  if (isLogin) {
    enhanceLogin();
    return;
  }

  enhanceHeaderControls();
  enhanceDropzone();
  installTerminal();

  function enhanceHeaderControls() {
    const target = document.getElementById('targetSelect');
    if (target) {
      const labels = {
        kubevirt: 'Target · KubeVirt',
        kvm: 'Target · KVM',
        proxmox: 'Target · Proxmox',
        generic: 'Target · Generic',
      };
      [...target.options].forEach((option) => {
        if (labels[option.value]) option.textContent = labels[option.value];
      });
      target.title = 'Migration target';
    }
    const recent = document.getElementById('recentDisksToggle');
    if (recent) recent.title = 'Recently used VM disks';
  }

  function applyZyvorBranding() {
    const headerLogo = document.querySelector('.nebula-header__logo');
    if (headerLogo) {
      headerLogo.src = 'img/zyvor-logo.png';
      headerLogo.alt = 'Zyvor';
    }
    const loginLogo = document.querySelector('.login-card img');
    if (loginLogo) {
      loginLogo.src = 'img/zyvor-logo.png';
      loginLogo.alt = 'Zyvor';
    }
  }

  function enhanceLogin() {
    const title = document.querySelector('.login-card h1');
    const subtitle = document.getElementById('loginSubtitle');
    const card = document.querySelector('.login-card');
    if (title) title.textContent = 'Welcome to GuestKit';
    if (subtitle) subtitle.textContent = 'Sign in to inspect, understand, and move virtual machines with confidence.';
    if (card) card.setAttribute('aria-label', 'GuestKit sign in');
  }

  function enhanceDropzone() {
    const dropzone = document.getElementById('dropzone');
    if (!dropzone) return;

    dropzone.addEventListener('pointermove', (event) => {
      const box = dropzone.getBoundingClientRect();
      const x = Math.max(0, Math.min(100, ((event.clientX - box.left) / box.width) * 100));
      const y = Math.max(0, Math.min(100, ((event.clientY - box.top) / box.height) * 100));
      dropzone.style.setProperty('--pointer-x', `${x}%`);
      dropzone.style.setProperty('--pointer-y', `${y}%`);
    }, { passive: true });

    ['dragenter', 'dragover'].forEach((name) => {
      dropzone.addEventListener(name, () => dropzone.classList.add('is-dragging'));
    });
    ['dragleave', 'drop'].forEach((name) => {
      dropzone.addEventListener(name, () => dropzone.classList.remove('is-dragging'));
    });
  }

  function installTerminal() {
    if (document.getElementById('zyvorGuestKitTerminal')) return;

    const state = {
      entries: [],
      maxEntries: 5000,
      filters: new Set(['debug', 'info', 'success', 'warn', 'error']),
      query: '',
      follow: true,
      dragDepth: 0,
    };

    const terminal = el('section', 'zyvor-terminal');
    terminal.id = 'zyvorGuestKitTerminal';
    terminal.hidden = true;
    terminal.setAttribute('aria-label', 'GuestKit activity console');
    terminal.innerHTML = `
      <div class="zyvor-terminal__titlebar" data-terminal-drag-handle>
        <div class="zyvor-terminal__traffic" aria-label="Window controls">
          <button type="button" class="close" aria-label="Close console" data-terminal-close></button>
          <button type="button" class="minimize" aria-label="Minimize console" data-terminal-minimize></button>
          <button type="button" class="zoom" aria-label="Reset console size" data-terminal-reset></button>
        </div>
        <div class="zyvor-terminal__title">GuestKit — Activity Console</div>
        <span class="zyvor-terminal__badge">LIVE</span>
      </div>
      <div class="zyvor-terminal__toolbar">
        <label title="Open a local log file">Open log<input type="file" data-terminal-file accept=".log,.txt,.json,.jsonl,.ndjson,text/plain,application/json" multiple hidden></label>
        <button type="button" data-level="error">Errors</button>
        <button type="button" data-level="warn">Warnings</button>
        <button type="button" data-level="info">Info</button>
        <input class="zyvor-terminal__filter" data-terminal-filter type="search" placeholder="Filter output…" aria-label="Filter console output">
        <button type="button" data-terminal-follow class="is-active">Follow</button>
        <button type="button" data-terminal-copy>Copy</button>
        <button type="button" data-terminal-export>Export</button>
        <button type="button" data-terminal-clear>Clear</button>
      </div>
      <div class="zyvor-terminal__screen" data-terminal-screen tabindex="0">
        <div class="zyvor-terminal__empty" data-terminal-empty>
          <div><strong>GuestKit Console</strong>Drag .log, .txt, .json, or .jsonl here — or watch live browser activity.</div>
        </div>
      </div>
      <div class="zyvor-terminal__status">
        <span data-terminal-count>0 lines</span>
        <span>⌘/Ctrl + &#96; toggles console</span>
      </div>`;

    const launcher = el('button', 'zyvor-terminal-launcher');
    launcher.type = 'button';
    launcher.id = 'zyvorTerminalLauncher';
    launcher.innerHTML = '<span aria-hidden="true">›_</span><span>Console</span><kbd>⌘`</kbd>';
    launcher.setAttribute('aria-controls', terminal.id);
    launcher.setAttribute('aria-expanded', 'false');

    const overlay = el('div', 'zyvor-log-drop-overlay');
    overlay.id = 'zyvorLogDropOverlay';
    overlay.innerHTML = '<div class="zyvor-log-drop-overlay__card"><div class="zyvor-log-drop-overlay__icon">›_</div><h2>Inspect logs</h2><p>Drop log or JSON files anywhere to open them in the GuestKit console.</p></div>';

    document.body.append(terminal, launcher, overlay);

    const screen = terminal.querySelector('[data-terminal-screen]');
    const empty = terminal.querySelector('[data-terminal-empty]');
    const count = terminal.querySelector('[data-terminal-count]');
    const filterInput = terminal.querySelector('[data-terminal-filter]');
    const followButton = terminal.querySelector('[data-terminal-follow]');
    const fileInput = terminal.querySelector('[data-terminal-file]');

    restoreGeometry(terminal);
    makeDraggable(terminal, terminal.querySelector('[data-terminal-drag-handle]'));

    launcher.addEventListener('click', () => setOpen(terminal.hidden));
    terminal.querySelector('[data-terminal-close]').addEventListener('click', () => setOpen(false));
    terminal.querySelector('[data-terminal-minimize]').addEventListener('click', () => setOpen(false));
    terminal.querySelector('[data-terminal-reset]').addEventListener('click', resetGeometry);

    terminal.querySelectorAll('[data-level]').forEach((button) => {
      button.addEventListener('click', () => {
        const level = button.dataset.level;
        if (button.classList.toggle('is-active')) {
          state.filters = new Set([level]);
          terminal.querySelectorAll('[data-level]').forEach((other) => {
            if (other !== button) other.classList.remove('is-active');
          });
        } else {
          state.filters = new Set(['debug', 'info', 'success', 'warn', 'error']);
        }
        render();
      });
    });

    filterInput.addEventListener('input', () => {
      state.query = filterInput.value.trim().toLowerCase();
      render();
    });

    followButton.addEventListener('click', () => {
      state.follow = !state.follow;
      followButton.classList.toggle('is-active', state.follow);
      if (state.follow) scrollToEnd();
    });

    terminal.querySelector('[data-terminal-clear]').addEventListener('click', () => {
      state.entries.length = 0;
      render();
    });

    terminal.querySelector('[data-terminal-copy]').addEventListener('click', async () => {
      const text = visibleEntries().map(formatPlain).join('\n');
      try {
        await navigator.clipboard.writeText(text);
        addEntry('success', 'Copied visible console output to clipboard.');
      } catch (error) {
        addEntry('warn', `Clipboard unavailable: ${error?.message || error}`);
      }
    });

    terminal.querySelector('[data-terminal-export]').addEventListener('click', () => {
      const text = visibleEntries().map(formatPlain).join('\n');
      const blob = new Blob([text], { type: 'text/plain;charset=utf-8' });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = `guestkit-console-${new Date().toISOString().replace(/[:.]/g, '-')}.log`;
      document.body.appendChild(anchor);
      anchor.click();
      anchor.remove();
      URL.revokeObjectURL(url);
    });

    fileInput.addEventListener('change', () => {
      if (fileInput.files?.length) readLogFiles([...fileInput.files]);
      fileInput.value = '';
    });

    screen.addEventListener('dragover', (event) => {
      if (!hasLogFiles(event.dataTransfer)) return;
      event.preventDefault();
      screen.classList.add('is-drop-target');
    });
    screen.addEventListener('dragleave', () => screen.classList.remove('is-drop-target'));
    screen.addEventListener('drop', (event) => {
      if (!hasLogFiles(event.dataTransfer)) return;
      event.preventDefault();
      screen.classList.remove('is-drop-target');
      readLogFiles([...event.dataTransfer.files]);
    });

    document.addEventListener('keydown', (event) => {
      if ((event.metaKey || event.ctrlKey) && event.key === '`') {
        event.preventDefault();
        setOpen(terminal.hidden);
      }
      if (event.key === 'Escape' && !terminal.hidden && terminal.contains(document.activeElement)) {
        setOpen(false);
      }
    });

    installGlobalLogDrop();
    bridgeConsole();
    bridgeErrors();

    addEntry('success', 'GuestKit Zyvor console ready.');
    addEntry('info', 'Drop log files here or press ⌘/Ctrl + ` to toggle the console.');

    function setOpen(open) {
      terminal.hidden = !open;
      launcher.setAttribute('aria-expanded', String(open));
      if (open) {
        screen.focus({ preventScroll: true });
        scrollToEnd();
      }
    }

    function addEntry(level, message, source = 'app', timestamp = new Date(), renderNow = true) {
      pushEntry(level, message, source, timestamp);
      trimEntries();
      if (renderNow) render();
    }

    function addEntries(entries) {
      entries.forEach((entry) => pushEntry(entry.level, entry.message, entry.source || 'app', entry.timestamp || new Date()));
      trimEntries();
      render();
    }

    function pushEntry(level, message, source, timestamp) {
      const normalized = normalizeLevel(level, message);
      state.entries.push({
        id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
        level: normalized,
        message: stringify(message),
        source,
        timestamp: timestamp instanceof Date && !Number.isNaN(timestamp.valueOf()) ? timestamp : new Date(),
      });
    }

    function trimEntries() {
      if (state.entries.length > state.maxEntries) {
        state.entries.splice(0, state.entries.length - state.maxEntries);
      }
    }

    function render() {
      const entries = visibleEntries();
      screen.querySelectorAll('.zyvor-terminal__line').forEach((node) => node.remove());
      empty.hidden = entries.length > 0;
      const fragment = document.createDocumentFragment();
      entries.forEach((entry) => fragment.appendChild(renderEntry(entry)));
      screen.appendChild(fragment);
      count.textContent = `${entries.length} ${entries.length === 1 ? 'line' : 'lines'}`;
      if (state.follow) scrollToEnd();
    }

    function visibleEntries() {
      return state.entries.filter((entry) => {
        if (!state.filters.has(entry.level)) return false;
        if (!state.query) return true;
        return `${entry.level} ${entry.source} ${entry.message}`.toLowerCase().includes(state.query);
      });
    }

    function renderEntry(entry) {
      const line = el('div', 'zyvor-terminal__line');
      line.dataset.level = entry.level;
      const time = el('span', 'zyvor-terminal__time');
      time.textContent = formatTime(entry.timestamp);
      const level = el('span', 'zyvor-terminal__level');
      level.textContent = labelLevel(entry.level);
      const message = el('span', 'zyvor-terminal__message');
      appendHighlighted(message, entry.message);
      line.append(time, level, message);
      return line;
    }

    function scrollToEnd() {
      requestAnimationFrame(() => { screen.scrollTop = screen.scrollHeight; });
    }

    async function readLogFiles(files) {
      const accepted = files.filter(isLogFile);
      if (!accepted.length) return;
      setOpen(true);
      const batch = [];
      for (const file of accepted) {
        try {
          const text = await readTextSafely(file, 4 * 1024 * 1024);
          const parsed = parseLogText(text, file.name).map((entry) => ({ ...entry, source: file.name }));
          batch.push(
            { level: 'success', message: `Opened ${file.name} (${formatBytes(file.size)})`, source: 'file', timestamp: new Date() },
            ...parsed,
          );
        } catch (error) {
          batch.push({ level: 'error', message: `Could not read ${file.name}: ${error?.message || error}`, source: 'file', timestamp: new Date() });
        }
      }
      if (batch.length) addEntries(batch);
    }

    function installGlobalLogDrop() {
      const stopLegacyHandlers = (event) => {
        event.preventDefault();
        event.stopImmediatePropagation();
      };

      window.addEventListener('dragenter', (event) => {
        if (!hasLogFiles(event.dataTransfer)) return;
        stopLegacyHandlers(event);
        state.dragDepth += 1;
        overlay.classList.add('is-visible');
      }, true);

      window.addEventListener('dragover', (event) => {
        if (!hasLogFiles(event.dataTransfer)) return;
        stopLegacyHandlers(event);
        if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy';
      }, true);

      window.addEventListener('dragleave', (event) => {
        if (!hasLogFiles(event.dataTransfer)) return;
        event.stopImmediatePropagation();
        state.dragDepth = Math.max(0, state.dragDepth - 1);
        if (state.dragDepth === 0) overlay.classList.remove('is-visible');
      }, true);

      window.addEventListener('drop', (event) => {
        if (!hasLogFiles(event.dataTransfer)) return;
        const files = [...(event.dataTransfer?.files || [])].filter(isLogFile);
        if (!files.length) return;
        stopLegacyHandlers(event);
        state.dragDepth = 0;
        overlay.classList.remove('is-visible');
        readLogFiles(files);
      }, true);
    }

    function bridgeConsole() {
      const methods = { log: 'info', info: 'info', debug: 'debug', warn: 'warn', error: 'error' };
      Object.entries(methods).forEach(([name, level]) => {
        const original = console[name]?.bind(console);
        if (!original || original.__guestkitZyvorWrapped) return;
        const wrapped = (...args) => {
          original(...args);
          try { addEntry(level, args.map(stringify).join(' '), 'browser'); } catch (_) {}
        };
        wrapped.__guestkitZyvorWrapped = true;
        console[name] = wrapped;
      });
    }

    function bridgeErrors() {
      window.addEventListener('error', (event) => {
        addEntry('error', `${event.message || 'Unhandled error'}${event.filename ? ` — ${shortPath(event.filename)}:${event.lineno || 0}` : ''}`, 'window');
      });
      window.addEventListener('unhandledrejection', (event) => {
        addEntry('error', `Unhandled promise rejection: ${stringify(event.reason)}`, 'window');
      });
    }

    function makeDraggable(panel, handle) {
      if (!handle) return;
      let active = false;
      let pointerId = null;
      let startX = 0;
      let startY = 0;
      let startLeft = 0;
      let startTop = 0;

      handle.addEventListener('pointerdown', (event) => {
        if (event.target.closest('button')) return;
        const rect = panel.getBoundingClientRect();
        active = true;
        pointerId = event.pointerId;
        startX = event.clientX;
        startY = event.clientY;
        startLeft = rect.left;
        startTop = rect.top;
        panel.style.left = `${rect.left}px`;
        panel.style.top = `${rect.top}px`;
        panel.style.right = 'auto';
        panel.style.bottom = 'auto';
        handle.setPointerCapture?.(pointerId);
      });

      handle.addEventListener('pointermove', (event) => {
        if (!active || event.pointerId !== pointerId) return;
        const maxLeft = Math.max(8, window.innerWidth - panel.offsetWidth - 8);
        const maxTop = Math.max(8, window.innerHeight - panel.offsetHeight - 8);
        const left = clamp(startLeft + event.clientX - startX, 8, maxLeft);
        const top = clamp(startTop + event.clientY - startY, 8, maxTop);
        panel.style.left = `${left}px`;
        panel.style.top = `${top}px`;
      });

      const end = (event) => {
        if (!active || event.pointerId !== pointerId) return;
        active = false;
        handle.releasePointerCapture?.(pointerId);
        saveGeometry(panel);
      };
      handle.addEventListener('pointerup', end);
      handle.addEventListener('pointercancel', end);

      if ('ResizeObserver' in window) {
        const observer = new ResizeObserver(() => saveGeometry(panel));
        observer.observe(panel);
      }
    }

    function saveGeometry(panel) {
      if (window.innerWidth < 620) return;
      const rect = panel.getBoundingClientRect();
      try {
        localStorage.setItem('guestkit.zyvorTerminal.geometry', JSON.stringify({
          left: Math.round(rect.left), top: Math.round(rect.top), width: Math.round(rect.width), height: Math.round(rect.height),
        }));
      } catch (_) {}
    }

    function restoreGeometry(panel) {
      if (window.innerWidth < 620) return;
      try {
        const saved = JSON.parse(localStorage.getItem('guestkit.zyvorTerminal.geometry') || 'null');
        if (!saved) return;
        panel.style.width = `${clamp(saved.width || 760, 360, window.innerWidth - 16)}px`;
        panel.style.height = `${clamp(saved.height || 430, 230, window.innerHeight - 16)}px`;
        panel.style.left = `${clamp(saved.left || 24, 8, window.innerWidth - 380)}px`;
        panel.style.top = `${clamp(saved.top || 80, 8, window.innerHeight - 250)}px`;
        panel.style.right = 'auto';
        panel.style.bottom = 'auto';
      } catch (_) {}
    }

    function resetGeometry() {
      try { localStorage.removeItem('guestkit.zyvorTerminal.geometry'); } catch (_) {}
      terminal.removeAttribute('style');
      terminal.hidden = false;
    }
  }

  function parseLogText(text, source) {
    const lines = String(text).replace(/\r\n/g, '\n').split('\n');
    return lines.slice(0, 5000).filter((line) => line.length).map((line) => {
      let message = line;
      let timestamp = new Date();
      let level = normalizeLevel('', line);

      const trimmed = line.trim();
      if ((trimmed.startsWith('{') && trimmed.endsWith('}')) || (source && /\.jsonl?$|\.ndjson$/i.test(source))) {
        try {
          const obj = JSON.parse(trimmed);
          const rawLevel = obj.level || obj.severity || obj.log_level || obj.priority || '';
          const rawTime = obj.timestamp || obj.time || obj.ts || obj.datetime;
          const rawMessage = obj.message || obj.msg || obj.event || obj.error;
          level = normalizeLevel(rawLevel, rawMessage || line);
          if (rawTime) {
            const parsed = new Date(rawTime);
            if (!Number.isNaN(parsed.valueOf())) timestamp = parsed;
          }
          message = rawMessage ? `${rawMessage}  ${compactObject(obj, ['message', 'msg', 'event', 'error', 'level', 'severity', 'log_level', 'priority', 'timestamp', 'time', 'ts', 'datetime'])}`.trim() : line;
        } catch (_) {}
      } else {
        const timestampMatch = line.match(/^(\d{4}-\d{2}-\d{2}[T ][0-9:.+-]+Z?)/);
        if (timestampMatch) {
          const parsed = new Date(timestampMatch[1]);
          if (!Number.isNaN(parsed.valueOf())) timestamp = parsed;
        }
      }
      return { level, message, timestamp };
    });
  }

  function appendHighlighted(target, text) {
    // Safe highlighter: never injects user-controlled HTML.
    const value = String(text);
    const pattern = /(\b[A-Za-z_][A-Za-z0-9_.-]*)(=|:\s)("[^"]*"|'[^']*'|\b\d+(?:\.\d+)?\b|\btrue\b|\bfalse\b|\bnull\b)/g;
    let last = 0;
    let match;
    while ((match = pattern.exec(value))) {
      target.appendChild(document.createTextNode(value.slice(last, match.index)));
      const key = el('span', 'token-key');
      key.textContent = match[1];
      target.appendChild(key);
      target.appendChild(document.createTextNode(match[2]));
      const val = el('span', /^['"]/.test(match[3]) ? 'token-string' : 'token-number');
      val.textContent = match[3];
      target.appendChild(val);
      last = match.index + match[0].length;
    }
    target.appendChild(document.createTextNode(value.slice(last)));
  }

  function normalizeLevel(raw, message = '') {
    const value = `${raw} ${message}`.toLowerCase();
    if (/\b(fatal|panic|error|err|failed|failure|exception|critical|crit)\b/.test(value)) return 'error';
    if (/\b(warn|warning|degraded|retry|timeout)\b/.test(value)) return 'warn';
    if (/\b(success|succeeded|ready|complete|completed|healthy|passed|ok)\b/.test(value)) return 'success';
    if (/\b(debug|trace|verbose)\b/.test(value)) return 'debug';
    return 'info';
  }

  function labelLevel(level) {
    return ({ debug: 'DEBUG', info: 'INFO', success: 'OK', warn: 'WARN', error: 'ERR' })[level] || 'INFO';
  }

  function formatPlain(entry) {
    return `${entry.timestamp.toISOString()} ${labelLevel(entry.level).padEnd(5)} ${entry.source ? `[${entry.source}] ` : ''}${entry.message}`;
  }

  function formatTime(date) {
    return date.toLocaleTimeString([], { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' });
  }

  function stringify(value) {
    if (typeof value === 'string') return value;
    if (value instanceof Error) return value.stack || `${value.name}: ${value.message}`;
    try { return JSON.stringify(value); } catch (_) { return String(value); }
  }

  function compactObject(obj, omit = []) {
    const clone = {};
    Object.keys(obj || {}).forEach((key) => { if (!omit.includes(key)) clone[key] = obj[key]; });
    if (!Object.keys(clone).length) return '';
    try { return JSON.stringify(clone); } catch (_) { return '';
    }
  }

  function transferHasFiles(dataTransfer) {
    if (!dataTransfer) return false;
    const types = dataTransfer.types;
    if (!types) return Boolean(dataTransfer.files?.length);
    if (typeof types.includes === 'function' && types.includes('Files')) return true;
    if (typeof types.indexOf === 'function' && types.indexOf('Files') !== -1) return true;
    if (typeof types.contains === 'function' && types.contains('Files')) return true;
    try { if (Array.from(types).includes('Files')) return true; } catch (_) {}
    return Boolean(dataTransfer.files?.length);
  }

  function hasLogFiles(dataTransfer) {
    if (!transferHasFiles(dataTransfer)) return false;
    const files = [...(dataTransfer.files || [])];
    if (files.length) return files.some(isLogFile);
    const items = [...(dataTransfer.items || [])];
    if (!items.length) return false;
    return items.some((item) => {
      if (item.kind !== 'file') return false;
      const file = item.getAsFile?.();
      return isLogName(file?.name || '') || /^text\//.test(item.type || '') || item.type === 'application/json';
    });
  }

  function isLogFile(file) {
    return Boolean(file && (isLogName(file.name) || /^text\//.test(file.type) || file.type === 'application/json'));
  }

  function isLogName(name) {
    return /\.(log|txt|json|jsonl|ndjson|out)$/i.test(name || '');
  }

  async function readTextSafely(file, maxBytes) {
    const blob = file.size > maxBytes ? file.slice(file.size - maxBytes) : file;
    const text = await blob.text();
    return file.size > maxBytes ? `[GuestKit: showing the last ${formatBytes(maxBytes)} of ${formatBytes(file.size)}]\n${text}` : text;
  }

  function formatBytes(bytes) {
    if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB'];
    const index = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
    return `${(bytes / (1024 ** index)).toFixed(index ? 1 : 0)} ${units[index]}`;
  }

  function shortPath(path) {
    try { return new URL(path, window.location.href).pathname.split('/').slice(-2).join('/'); } catch (_) { return path; }
  }

  function clamp(value, min, max) { return Math.min(max, Math.max(min, value)); }

  function el(tag, className) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    return node;
  }
})();
