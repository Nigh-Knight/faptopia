// ==UserScript==
// @name         Faptopia 4chan Launcher
// @namespace    https://github.com/faptopia
// @version      1.0.0
// @description  Builds a faptopia 4chan command from your watched/pinned /gif/ threads
// @author       faptopia
// @match        *://boards.4chan.org/*
// @match        *://boards.4channel.org/*
// @grant        GM_setClipboard
// @run-at       document-idle
// ==/UserScript==

(function () {
    'use strict';

    // ===== THREAD ID COLLECTION =====

    // Read thread IDs from pinned threads only.
    // .pinned is the pin icon img tag; its parent <a> has the thread href.
    function collectGifThreadIds() {
        const ids = new Map(); // id -> 'pinned'

        document.querySelectorAll('img.pinned').forEach(img => {
            const a = img.closest('a[href]');
            if (!a) return;
            const m = a.href.match(/\/gif\/thread\/(\d+)/);
            if (m) ids.set(m[1], 'pinned');
        });

        return ids;
    }

    function partitionIds(idMap) {
        const pinned = [];
        idMap.forEach((_, id) => pinned.push(id));
        return { watched: pinned };
    }

    // ===== UI =====

    const STYLES = `
        #fap-launcher {
            position: fixed;
            bottom: 20px;
            right: 20px;
            z-index: 999999;
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
        }
        #fap-btn {
            background: #0f9d58;
            color: #fff;
            border: none;
            padding: 9px 16px;
            border-radius: 8px;
            cursor: pointer;
            font-size: 13px;
            font-weight: 600;
            box-shadow: 0 2px 12px rgba(0,0,0,0.4);
            display: flex;
            align-items: center;
            gap: 7px;
            transition: background 0.15s;
        }
        #fap-btn:hover { background: #0b8043; }
        #fap-badge {
            background: rgba(0,0,0,0.3);
            border-radius: 10px;
            padding: 1px 7px;
            font-size: 11px;
            font-weight: 700;
        }
        #fap-panel {
            display: none;
            position: absolute;
            bottom: calc(100% + 10px);
            right: 0;
            width: 340px;
            background: #1a1a1a;
            border: 1px solid rgba(255,255,255,0.1);
            border-radius: 10px;
            box-shadow: 0 8px 30px rgba(0,0,0,0.6);
            overflow: hidden;
        }
        #fap-panel.open { display: block; }
        .fap-header {
            background: #111;
            padding: 10px 14px;
            font-size: 12px;
            font-weight: 700;
            color: #aaa;
            letter-spacing: 0.08em;
            text-transform: uppercase;
            border-bottom: 1px solid rgba(255,255,255,0.06);
        }
        .fap-section-label {
            padding: 8px 14px 4px;
            font-size: 10px;
            font-weight: 600;
            color: #555;
            text-transform: uppercase;
            letter-spacing: 0.1em;
        }
        .fap-thread-list {
            max-height: 180px;
            overflow-y: auto;
            padding: 4px 0;
        }
        .fap-thread {
            display: flex;
            align-items: center;
            gap: 8px;
            padding: 5px 14px;
            cursor: pointer;
            transition: background 0.1s;
        }
        .fap-thread:hover { background: rgba(255,255,255,0.04); }
        .fap-thread input[type=checkbox] {
            width: 14px; height: 14px;
            accent-color: #0f9d58;
            cursor: pointer;
            flex-shrink: 0;
        }
        .fap-thread-id {
            font-family: monospace;
            font-size: 12px;
            color: #ccc;
        }
        .fap-thread-src {
            margin-left: auto;
            font-size: 10px;
            color: #555;
            text-transform: uppercase;
            letter-spacing: 0.06em;
        }
        .fap-empty {
            padding: 12px 14px;
            font-size: 12px;
            color: #555;
            font-style: italic;
        }
        .fap-cmd-box {
            padding: 8px 14px;
            border-top: 1px solid rgba(255,255,255,0.06);
        }
        .fap-cmd-text {
            font-family: monospace;
            font-size: 11px;
            color: #888;
            word-break: break-all;
            background: #111;
            padding: 6px 8px;
            border-radius: 5px;
            border: 1px solid rgba(255,255,255,0.06);
            margin-bottom: 8px;
            min-height: 28px;
        }
        .fap-actions {
            display: flex;
            gap: 6px;
        }
        .fap-copy-btn {
            flex: 1;
            background: #0f9d58;
            color: #fff;
            border: none;
            padding: 8px;
            border-radius: 6px;
            cursor: pointer;
            font-size: 12px;
            font-weight: 600;
            transition: background 0.15s;
        }
        .fap-copy-btn:hover { background: #0b8043; }
        .fap-copy-btn:disabled { background: #333; color: #666; cursor: default; }
        .fap-catalog-btn {
            background: #2a2a2a;
            color: #aaa;
            border: 1px solid rgba(255,255,255,0.08);
            padding: 8px 10px;
            border-radius: 6px;
            cursor: pointer;
            font-size: 11px;
            font-weight: 600;
            transition: background 0.15s;
            white-space: nowrap;
        }
        .fap-catalog-btn:hover { background: #333; color: #fff; }
        .fap-toast {
            position: fixed;
            bottom: 80px;
            left: 50%;
            transform: translateX(-50%);
            background: #0f9d58;
            color: #fff;
            padding: 9px 20px;
            border-radius: 8px;
            font-size: 13px;
            font-weight: 600;
            z-index: 1000000;
            pointer-events: none;
            opacity: 0;
            transition: opacity 0.2s;
        }
        .fap-toast.show { opacity: 1; }
    `;

    function injectStyles() {
        const style = document.createElement('style');
        style.textContent = STYLES;
        document.head.appendChild(style);
    }

    // Build the command string from checked IDs
    function buildCommand(ids) {
        if (ids.length === 0) return '';
        return `faptopia 4chan ${ids.join(' ')}`;
    }

    // Show a brief toast notification
    let toastTimer;
    function showToast(msg) {
        let toast = document.getElementById('fap-toast-el');
        if (!toast) {
            toast = document.createElement('div');
            toast.id = 'fap-toast-el';
            toast.className = 'fap-toast';
            document.body.appendChild(toast);
        }
        toast.textContent = msg;
        toast.classList.add('show');
        clearTimeout(toastTimer);
        toastTimer = setTimeout(() => toast.classList.remove('show'), 2000);
    }

    function copyToClipboard(text) {
        if (typeof GM_setClipboard !== 'undefined') {
            GM_setClipboard(text);
            return;
        }
        navigator.clipboard.writeText(text).catch(() => {
            // Fallback: temporary textarea
            const ta = document.createElement('textarea');
            ta.value = text;
            ta.style.cssText = 'position:fixed;left:-9999px';
            document.body.appendChild(ta);
            ta.select();
            document.execCommand('copy');
            ta.remove();
        });
    }

    // Render thread rows into .fap-thread-list
    function renderThreads(container, threads, source, checkedSet) {
        threads.forEach(id => {
            const row = document.createElement('label');
            row.className = 'fap-thread';

            const cb = document.createElement('input');
            cb.type = 'checkbox';
            cb.checked = checkedSet.has(id);
            cb.dataset.id = id;
            cb.addEventListener('change', () => {
                if (cb.checked) checkedSet.add(id);
                else checkedSet.delete(id);
                updatePanel();
            });

            const idSpan = document.createElement('span');
            idSpan.className = 'fap-thread-id';
            idSpan.textContent = id;

            const srcSpan = document.createElement('span');
            srcSpan.className = 'fap-thread-src';
            srcSpan.textContent = source;

            row.appendChild(cb);
            row.appendChild(idSpan);
            row.appendChild(srcSpan);
            container.appendChild(row);
        });
    }

    // State
    let checkedIds = new Set();
    let panelOpen = false;

    function updatePanel() {
        const cmdText = document.getElementById('fap-cmd-text');
        const copyBtn = document.getElementById('fap-copy-btn');
        const badge = document.getElementById('fap-badge');
        if (!cmdText) return;

        const ids = Array.from(checkedIds);
        const cmd = buildCommand(ids);
        cmdText.textContent = cmd ? cmd + ' --deploy' : 'No pinned threads selected';
        copyBtn.disabled = ids.length === 0;
        const deployBtnEl = document.getElementById('fap-deploy-btn');
        if (deployBtnEl) deployBtnEl.disabled = ids.length === 0;
        badge.textContent = checkedIds.size;
    }

    function buildPanel(idMap) {
        const { watched } = partitionIds(idMap);

        const panel = document.getElementById('fap-panel');
        panel.innerHTML = '';

        const header = document.createElement('div');
        header.className = 'fap-header';
        header.textContent = 'Faptopia /gif/ Launcher';
        panel.appendChild(header);

        // Watched threads section
        const watchedList = document.createElement('div');
        watchedList.className = 'fap-thread-list';

        if (watched.length > 0) {
            const label = document.createElement('div');
            label.className = 'fap-section-label';
            label.textContent = `Pinned (${watched.length})`;
            panel.appendChild(label);
            renderThreads(watchedList, watched, 'pinned', checkedIds);
        } else {
            const empty = document.createElement('div');
            empty.className = 'fap-empty';
            empty.textContent = 'No pinned /gif/ threads found. Pin threads on the catalog or watcher.';
            watchedList.appendChild(empty);
            const label = document.createElement('div');
            label.className = 'fap-section-label';
            label.textContent = 'Pinned';
            panel.appendChild(label);
        }
        panel.appendChild(watchedList);

        // Command box + actions
        const cmdBox = document.createElement('div');
        cmdBox.className = 'fap-cmd-box';

        const cmdText = document.createElement('div');
        cmdText.className = 'fap-cmd-text';
        cmdText.id = 'fap-cmd-text';
        const ids = Array.from(checkedIds);
        cmdText.textContent = ids.length > 0
            ? buildCommand(ids) + ' --deploy'
            : 'No pinned threads selected';

        const actions = document.createElement('div');
        actions.className = 'fap-actions';

        // "Copy" — command without --deploy
        const copyBtn = document.createElement('button');
        copyBtn.className = 'fap-copy-btn fap-copy-plain';
        copyBtn.id = 'fap-copy-btn';
        copyBtn.textContent = 'Copy';
        copyBtn.disabled = ids.length === 0;
        copyBtn.style.cssText = 'flex:0 0 auto;background:#2a2a2a;border:1px solid rgba(255,255,255,0.1);';
        copyBtn.onclick = () => {
            const cmd = buildCommand(Array.from(checkedIds));
            if (!cmd) return;
            copyToClipboard(cmd);
            showToast('Copied');
            panel.classList.remove('open');
            panelOpen = false;
        };

        // "Copy + Deploy" — command with --deploy (default)
        const deployBtn = document.createElement('button');
        deployBtn.className = 'fap-copy-btn';
        deployBtn.id = 'fap-deploy-btn';
        deployBtn.textContent = 'Copy + Deploy';
        deployBtn.disabled = ids.length === 0;
        deployBtn.onclick = () => {
            const cmd = buildCommand(Array.from(checkedIds));
            if (!cmd) return;
            copyToClipboard(cmd + ' --deploy');
            showToast('Copied with --deploy');
            panel.classList.remove('open');
            panelOpen = false;
        };

        actions.appendChild(copyBtn);
        actions.appendChild(deployBtn);

        cmdBox.appendChild(cmdText);
        cmdBox.appendChild(actions);
        panel.appendChild(cmdBox);
    }

    function refreshPanel() {
        const idMap = collectGifThreadIds();
        buildPanel(idMap);
        updatePanel();
    }

    // ===== MOUNT =====

    function mount() {
        injectStyles();

        const launcher = document.createElement('div');
        launcher.id = 'fap-launcher';

        const btn = document.createElement('button');
        btn.id = 'fap-btn';
        btn.innerHTML = '<span>faptopia</span><span id="fap-badge" class="fap-badge">0</span>';
        btn.title = 'Build faptopia 4chan command from watched /gif/ threads';

        const panel = document.createElement('div');
        panel.id = 'fap-panel';

        btn.onclick = (e) => {
            e.stopPropagation();
            panelOpen = !panelOpen;
            if (panelOpen) {
                refreshPanel();
                panel.classList.add('open');
            } else {
                panel.classList.remove('open');
            }
        };

        document.addEventListener('click', (e) => {
            if (!launcher.contains(e.target) && panelOpen) {
                panel.classList.remove('open');
                panelOpen = false;
            }
        });

        launcher.appendChild(panel);
        launcher.appendChild(btn);
        document.body.appendChild(launcher);

        // Initial sync
        syncPinned();

        // Watch for pin/unpin changes in the DOM (debounced to avoid thrashing)
        let syncTimer;
        new MutationObserver(() => {
            clearTimeout(syncTimer);
            syncTimer = setTimeout(syncPinned, 300);
        }).observe(document.body, {
            subtree: true,
            childList: true,
            attributes: true,
            attributeFilter: ['class', 'src'],
        });
    }

    // Sync checkedIds with current .pinned state in the DOM.
    // Adds newly pinned threads, removes unpinned ones, updates badge.
    function syncPinned() {
        const idMap = collectGifThreadIds();
        const currentPinned = new Set(idMap.keys());

        // Add new pins
        currentPinned.forEach(id => {
            if (!checkedIds.has(id)) checkedIds.add(id);
        });

        // Remove unpinned threads
        checkedIds.forEach(id => {
            if (!currentPinned.has(id)) checkedIds.delete(id);
        });

        const badge = document.getElementById('fap-badge');
        if (badge) badge.textContent = checkedIds.size;

        // If panel is open, rebuild it to reflect the change
        if (panelOpen) {
            buildPanel(idMap);
            updatePanel();
        }
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', mount);
    } else {
        mount();
    }

})();
