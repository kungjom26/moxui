// moxui — Alpine.js SPA logic.
//
// State shape: see the `moxui()` factory below. We persist `token` and
// `theme` to localStorage so a page refresh doesn't kick the user back
// to the login screen. User details are NOT persisted — we re-fetch
// /api/v1/auth/me on every page load to validate the token is still good.
//
// Routes: see the `route` field. Hash-based (`#/vms`, `#/lxcs`, etc.)
// because we don't want to bother the backend with router history. Day
// 11+ adds proper VM detail routes (`#/vms/<cluster>/<vmid>`).

const VM_POLL_MS = 2000;          // poll VM list every 2s while view is active
const VM_DETAIL_POLL_MS = 2000;   // poll VM detail (Overview) every 2s
const VM_STALE_MS = 5000;         // mark data stale after 5s of no fresh fetch
const VM_RETRY_DELAYS = [2000, 4000, 8000, 15000, 30000]; // exponential backoff cap

function moxui() {
    return {
        // --- auth ---
        token: localStorage.getItem('moxui.token') || null,
        user: null,
        loginForm: { username: '', password: '' },
        loginError: null,
        loggingIn: false,

        // --- theme ---
        theme: localStorage.getItem('moxui.theme') || 'light',

        // --- routing ---
        route: this.parseRoute(),

        // --- data ---
        vms: null,                 // VmRow[] from /api/v1/vms
        vmsError: null,            // { message, retried } | null
        vmsLastUpdated: null,      // ms epoch of last successful fetch
        vmsRetryDelay: null,       // ms until next retry after error
        lxcs: null,
        storages: null,
        networks: null,
        auditEntries: [],
        auditTotal: 0,
        auditPage: 1,
        auditPerPage: 50,
        auditPages: 0,
        auditLoading: false,
        auditError: null,
        auditFilter: {
            method: '',
            path: '',
            status: '',
            request_id: '',
        },
        selectedVm: null,

        // --- VM detail page state ---
        vmDetailTab: 'overview',   // 'overview' | 'config' | 'tasks'
        vmConfig: null,            // VmConfig from /api/v1/vms/.../config
        vmConfigLoading: false,
        vmTasks: [],               // [{ upid, status, type, starttime, endtime }]
        vmTasksLoading: false,
        vmDetailError: null,
        vmDetailPollHandle: null,
        // Active action (Start/Stop/...) we fired and are tracking via /api/v1/tasks/...
        vmActionPending: null,      // { upid, action, startedAt } | null
        // Confirm dialog state for destructive / state-changing actions.
        vmConfirm: null,           // { action, vmid, vname, body, needsVmid, danger } | null
        vmConfirmInput: '',        // user-typed VMID (for delete confirmation)
        vmActionError: null,       // last action error (rendered as inline banner)

        // --- VM list UI state ---
        vmFilter: {
            search: '',
            status: '',            // '' = all, or 'running' / 'stopped' / 'paused'
            cluster: '',           // '' = all, or cluster name
            node: '',              // '' = all, or node name
        },
        vmSort: { key: 'vmid', dir: 'asc' },  // dir: 'asc' | 'desc'
        vmPollHandle: null,        // setInterval id for active polling

        // ----- lifecycle -----

        async init() {
            // Re-apply theme on every page load (the :class binding reads
            // `theme` but we also set <html> directly so the CSS variables
            // resolve before the first paint).
            document.documentElement.classList.toggle('dark', this.theme === 'dark');

            if (this.token) {
                try {
                    await this.fetchMe();
                    await this.fetchAll();
                } catch (e) {
                    // Token expired or invalid.
                    this.logout();
                }
            }

            window.addEventListener('hashchange', () => {
                this.route = this.parseRoute();
                this.maybeLoadRoute();
            });
            // Stop polling when the tab is hidden — saves backend cycles.
            document.addEventListener('visibilitychange', () => {
                if (document.hidden) {
                    this.stopVmPolling();
                } else if (this.route === 'vms' && this.token) {
                    this.startVmPolling();
                }
            });

            this.maybeLoadRoute();

            // Keyboard shortcuts: g+v / g+l / g+s / g+n = jump to section.
            // Matches the Phase 1 spec's `g+d` / `g+v` pattern.
            let prefix = null;
            document.addEventListener('keydown', (e) => {
                if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') return;
                if (prefix && !e.metaKey && !e.ctrlKey && !e.altKey) {
                    const map = { v: 'vms', l: 'lxcs', s: 'storages', n: 'networks', a: 'audit', d: 'vms' };
                    if (map[e.key]) { location.hash = '#/' + map[e.key]; prefix = null; return; }
                }
                if (e.key === 'g') { prefix = 'g'; setTimeout(() => { prefix = null; }, 800); }
            });
        },

        parseRoute() {
            const hash = location.hash.replace(/^#\/?/, '');
            if (hash.startsWith('vm/')) return 'vm-detail';
            return hash || 'vms';
        },

        parseVmDetail() {
            // Hash like "#/vm/<cluster>/<node>/<vmid>" → { cluster, node, vmid }.
            // Falls back to the cached list to find node if the URL didn't carry it
            // (e.g. legacy links from before Day 12).
            const m = location.hash.match(/^#\/vm\/([^/]+)\/(?:([^/]+)\/)?(\d+)/);
            if (!m) return null;
            const cluster = m[1];
            const vmid = Number(m[3]);
            let node = m[2] || null;
            if (!node) {
                const cached = (this.vms || []).find(v => v.cluster === cluster && v.vmid === vmid);
                node = cached ? cached.node : null;
            }
            return node ? { cluster, node, vmid } : null;
        },

        maybeLoadRoute() {
            if (!this.token) return;
            switch (this.route) {
                case 'vms':
                    this.stopVmDetailPolling();
                    this.startVmPolling();
                    this.fetchVms();
                    break;
                case 'lxcs':
                    this.stopVmPolling();
                    this.stopVmDetailPolling();
                    this.fetchLxcs();
                    break;
                case 'storages':
                    this.stopVmPolling();
                    this.stopVmDetailPolling();
                    this.fetchStorages();
                    break;
                case 'networks':
                    this.stopVmPolling();
                    this.stopVmDetailPolling();
                    this.fetchNetworks();
                    break;
                case 'audit':
                    this.stopVmPolling();
                    this.stopVmDetailPolling();
                    this.fetchAudit();
                    break;
                case 'vm-detail': {
                    this.stopVmPolling();
                    const sel = this.parseVmDetail();
                    if (sel) {
                        this.selectedVm = { cluster: sel.cluster, vmid: sel.vmid, node: sel.node };
                        // Try to populate from the cached list (we may have it).
                        const cached = (this.vms || []).find(v =>
                            v.cluster === sel.cluster && v.vmid === sel.vmid
                        );
                        if (cached) Object.assign(this.selectedVm, cached);
                    }
                    this.fetchVmDetail();
                    this.startVmDetailPolling();
                    break;
                }
            }
        },

        // ----- auth -----

        async login() {
            this.loginError = null;
            this.loggingIn = true;
            try {
                const resp = await fetch('/api/v1/auth/login', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(this.loginForm),
                });
                if (!resp.ok) {
                    const err = await resp.json().catch(() => ({}));
                    this.loginError = err.message || err.error || `Login failed (${resp.status})`;
                    return;
                }
                const { token } = await resp.json();
                this.token = token;
                localStorage.setItem('moxui.token', token);
                await this.fetchMe();
                await this.fetchAll();
                this.route = 'vms';
                location.hash = '#/vms';
            } catch (e) {
                this.loginError = String(e);
            } finally {
                this.loggingIn = false;
            }
        },

        async fetchMe() {
            const resp = await this.api('/api/v1/auth/me');
            this.user = await resp.json();
        },

        async api(path, init = {}) {
            const headers = { ...(init.headers || {}) };
            if (this.token) headers['Authorization'] = 'Bearer ' + this.token;
            const resp = await fetch(path, { ...init, headers });
            if (resp.status === 401) { this.logout(); throw new Error('unauthorized'); }
            if (!resp.ok) {
                const err = await resp.json().catch(() => ({}));
                throw new Error(err.message || err.error || `${path} → HTTP ${resp.status}`);
            }
            return resp;
        },

        logout() {
            this.token = null;
            this.user = null;
            localStorage.removeItem('moxui.token');
            this.vms = this.lxcs = this.storages = this.networks = null;
            this.auditEntries = [];
            this.auditTotal = 0;
            this.auditPages = 0;
            this.auditError = null;
            this.vmsError = null;
            this.vmsLastUpdated = null;
            this.stopVmPolling();
            location.hash = '';
            this.route = 'vms';
        },

        toggleTheme() {
            this.theme = this.theme === 'dark' ? 'light' : 'dark';
            localStorage.setItem('moxui.theme', this.theme);
            document.documentElement.classList.toggle('dark', this.theme === 'dark');
        },

        // ----- data fetch -----

        async fetchAll() {
            await Promise.all([
                this.fetchVms(),
                this.fetchLxcs(),
                this.fetchStorages(),
                this.fetchNetworks(),
            ]);
        },

        async fetchLxcs()     { this.lxcs     = (await (await this.api('/api/v1/lxcs'    )).json()).lxcs; },
        async fetchStorages() { this.storages = (await (await this.api('/api/v1/storages')).json()).storages; },
        async fetchNetworks() { this.networks = (await (await this.api('/api/v1/networks')).json()).networks; },

        // ----- Audit log -----

        async fetchAudit() {
            this.auditLoading = true;
            this.auditError = null;
            try {
                const params = new URLSearchParams();
                params.set('page', this.auditPage);
                params.set('per_page', this.auditPerPage);
                if (this.auditFilter.method) params.set('method', this.auditFilter.method);
                if (this.auditFilter.path) params.set('path', this.auditFilter.path);
                if (this.auditFilter.status) params.set('status', this.auditFilter.status);
                if (this.auditFilter.request_id) params.set('request_id', this.auditFilter.request_id);

                const resp = await this.api('/api/v1/audit?' + params.toString());
                const body = await resp.json();
                this.auditEntries = body.entries || [];
                this.auditTotal = body.total || 0;
                this.auditPage = body.page || 1;
                this.auditPerPage = body.per_page || 50;
                this.auditPages = body.pages || 0;
            } catch (e) {
                this.auditError = e.message || String(e);
                this.auditEntries = [];
            } finally {
                this.auditLoading = false;
            }
        },

        auditGoToPage(page) {
            this.auditPage = page;
            this.fetchAudit();
        },

        clearAuditFilters() {
            this.auditFilter = { method: '', path: '', status: '', request_id: '' };
            this.auditPage = 1;
            this.fetchAudit();
        },

        // ----- VM list polling + fetch -----

        async fetchVms() {
            try {
                const resp = await this.api('/api/v1/vms');
                const body = await resp.json();
                this.vms = body.vms || [];
                // Backend reports per-cluster errors. If any cluster failed,
                // surface as vmsError so the operator can see what went wrong
                // without losing the (possibly partial) data we did get.
                if (body.errors && Object.keys(body.errors).length > 0) {
                    this.vmsError = {
                        message: 'Some clusters failed: ' +
                                 Object.entries(body.errors)
                                       .map(([k, v]) => `${k}: ${v}`)
                                       .join('; '),
                        retried: false,
                    };
                } else {
                    this.vmsError = null;
                }
                this.vmsLastUpdated = Date.now();
                this.vmsRetryDelay = null;
            } catch (e) {
                // Don't clobber the existing list — keep showing stale data
                // with a visible error so operators don't lose situational
                // awareness if the backend hiccups briefly.
                const prev = this.vmsRetryDelay || 0;
                const idx = Math.min(Math.floor(prev / 2000), VM_RETRY_DELAYS.length - 1);
                this.vmsRetryDelay = VM_RETRY_DELAYS[idx];
                this.vmsError = {
                    message: e.message || String(e),
                    retried: true,
                    nextRetryMs: this.vmsRetryDelay,
                };
                if (this.vmsRetryDelay < VM_RETRY_DELAYS[VM_RETRY_DELAYS.length - 1]) {
                    this.vmsRetryDelay = VM_RETRY_DELAYS[Math.min(idx + 1, VM_RETRY_DELAYS.length - 1)];
                }
            }
        },

        startVmPolling() {
            this.stopVmPolling();
            this.vmPollHandle = setInterval(() => this.fetchVms(), VM_POLL_MS);
        },

        stopVmPolling() {
            if (this.vmPollHandle) {
                clearInterval(this.vmPollHandle);
                this.vmPollHandle = null;
            }
        },

        // ----- VM detail (Overview/Config/Tasks) -----

        async fetchVmDetail() {
            if (!this.selectedVm || !this.selectedVm.cluster || !this.selectedVm.node) {
                return;
            }
            try {
                const resp = await this.api(
                    `/api/v1/vms/${this.selectedVm.cluster}/${this.selectedVm.vmid}`
                );
                Object.assign(this.selectedVm, await resp.json());
                this.vmDetailError = null;
            } catch (e) {
                this.vmDetailError = { message: e.message || String(e) };
            }
        },

        async fetchVmConfig() {
            if (!this.selectedVm || !this.selectedVm.node) return;
            this.vmConfigLoading = true;
            try {
                const resp = await this.api(
                    `/api/v1/vms/${this.selectedVm.cluster}/${this.selectedVm.node}/${this.selectedVm.vmid}/config`
                );
                this.vmConfig = await resp.json();
            } catch (e) {
                this.vmConfig = null;
                this.vmDetailError = { message: `config: ${e.message || e}` };
            } finally {
                this.vmConfigLoading = false;
            }
        },

        startVmDetailPolling() {
            this.stopVmDetailPolling();
            this.vmDetailPollHandle = setInterval(() => this.fetchVmDetail(), VM_DETAIL_POLL_MS);
        },

        stopVmDetailPolling() {
            if (this.vmDetailPollHandle) {
                clearInterval(this.vmDetailPollHandle);
                this.vmDetailPollHandle = null;
            }
        },

        switchVmTab(tab) {
            this.vmDetailTab = tab;
            this.vmDetailError = null;
            if (tab === 'config' && !this.vmConfig) {
                this.fetchVmConfig();
            }
            if (tab === 'tasks') {
                this.fetchVmTask();
            }
        },

        async fetchVmTask() {
            if (!this.vmActionPending || !this.selectedVm) return;
            this.vmTasksLoading = true;
            try {
                const upid = encodeURIComponent(this.vmActionPending.upid);
                const resp = await this.api(
                    `/api/v1/tasks/${this.selectedVm.cluster}/${this.selectedVm.node}/${upid}`
                );
                const task = await resp.json();
                // Replace or insert into vmTasks list.
                const idx = this.vmTasks.findIndex(t => t.upid === task.upid);
                if (idx >= 0) this.vmTasks.splice(idx, 1, task);
                else this.vmTasks.unshift(task);
                // Clear pending once the task is no longer running.
                if (task.status !== 'running') {
                    this.vmActionPending = null;
                }
            } catch (e) {
                this.vmActionError = e.message || String(e);
            } finally {
                this.vmTasksLoading = false;
            }
        },

        // ----- VM actions (start/stop/shutdown/reboot/delete) -----

        requestVmAction(action, opts = {}) {
            // Open a confirm dialog. Destructive actions (delete) require
            // the operator to type the VMID before the button enables.
            const danger = (action === 'delete');
            this.vmActionError = null;
            this.vmConfirmInput = '';
            this.vmConfirm = {
                action,
                vmid: this.selectedVm.vmid,
                vname: this.selectedVm.name || `(vmid ${this.selectedVm.vmid})`,
                body: opts.body || null,           // for delete: { purge, force, skiplock }
                needsVmid: danger,
                danger,
            };
        },

        cancelVmAction() {
            this.vmConfirm = null;
            this.vmConfirmInput = '';
        },

        async confirmVmAction() {
            if (!this.vmConfirm) return;
            const { action, body } = this.vmConfirm;
            const { cluster, node, vmid } = this.selectedVm;
            this.vmActionError = null;
            try {
                const url = `/api/v1/vms/${cluster}/${node}/${vmid}/${action}`;
                const init = {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: body ? JSON.stringify(body) : null,
                };
                const resp = await this.api(url, init);
                const data = await resp.json();
                this.vmConfirm = null;
                this.vmConfirmInput = '';
                if (data.upid) {
                    this.vmActionPending = {
                        upid: data.upid,
                        action,
                        startedAt: Date.now(),
                    };
                    // Refresh detail immediately so the user sees the
                    // status change, then poll the task to track completion.
                    this.fetchVmDetail();
                    this.fetchVmTask();
                    if (this.vmDetailTab !== 'tasks') {
                        // Stay on current tab but make the banner visible.
                    }
                    if (action === 'delete') {
                        // After delete the VM is gone — bounce back to list.
                        setTimeout(() => {
                            location.hash = '#/vms';
                            this.route = 'vms';
                            this.fetchVms();
                        }, 1500);
                    }
                } else {
                    // No UPID returned (unexpected) — still refresh.
                    this.fetchVmDetail();
                }
            } catch (e) {
                this.vmActionError = e.message || String(e);
                // Keep the dialog open so the user can retry or cancel.
            }
        },

        // Per-action button state — disable when the action would be a no-op
        // (Start on a running VM, Stop on a stopped VM, etc.).
        canStart() { return this.selectedVm && this.selectedVm.status !== 'running'; },
        canStop()  { return this.selectedVm && (this.selectedVm.status === 'running' || this.selectedVm.status === 'paused'); },
        canShutdown() { return this.selectedVm && (this.selectedVm.status === 'running' || this.selectedVm.status === 'paused'); },
        canReboot()   { return this.selectedVm && this.selectedVm.status === 'running'; },
        canDelete()   { return this.selectedVm != null; },

        // ----- UI helpers -----

        openVm(vm) {
            this.selectedVm = vm;
            this.route = 'vm-detail';
            this.vmDetailTab = 'overview';
            this.vmConfig = null;
            this.vmTasks = [];
            this.vmActionPending = null;
            this.vmActionError = null;
            location.hash = `#/vm/${vm.cluster}/${vm.node}/${vm.vmid}`;
        },

        humanMemoryMiB(mib) {
            if (mib == null) return '—';
            if (mib >= 1024) return (mib / 1024).toFixed(1) + ' GB';
            return mib + ' MiB';
        },

        // ----- VM list: filter / sort / derived state -----

        get filteredVms() {
            if (!this.vms) return [];
            const q = (this.vmFilter.search || '').trim().toLowerCase();
            return this.vms.filter(vm => {
                if (this.vmFilter.status && vm.status !== this.vmFilter.status) return false;
                if (this.vmFilter.cluster && vm.cluster !== this.vmFilter.cluster) return false;
                if (this.vmFilter.node && vm.node !== this.vmFilter.node) return false;
                if (q) {
                    const hay = [
                        String(vm.vmid),
                        (vm.name || '').toLowerCase(),
                        (vm.tags || '').toLowerCase(),
                        vm.node.toLowerCase(),
                        vm.cluster.toLowerCase(),
                    ].join(' ');
                    if (!hay.includes(q)) return false;
                }
                return true;
            });
        },

        get sortedVms() {
            const list = this.filteredVms.slice();
            const { key, dir } = this.vmSort;
            const sign = dir === 'desc' ? -1 : 1;
            list.sort((a, b) => {
                let av = a[key], bv = b[key];
                // nulls last regardless of direction
                if (av == null && bv == null) return 0;
                if (av == null) return 1;
                if (bv == null) return -1;
                if (typeof av === 'number' && typeof bv === 'number') return (av - bv) * sign;
                return String(av).localeCompare(String(bv)) * sign;
            });
            return list;
        },

        get distinctClusters() {
            if (!this.vms) return [];
            return [...new Set(this.vms.map(v => v.cluster))].sort();
        },

        get distinctNodes() {
            if (!this.vms) return [];
            // Filter nodes by the selected cluster so the dropdown stays coherent.
            const filtered = this.vmFilter.cluster
                ? this.vms.filter(v => v.cluster === this.vmFilter.cluster)
                : this.vms;
            return [...new Set(filtered.map(v => v.node))].sort();
        },

        get vmsAreStale() {
            if (!this.vmsLastUpdated) return false;
            return (Date.now() - this.vmsLastUpdated) > VM_STALE_MS;
        },

        // Cycle: none → asc → desc → none
        sortBy(key) {
            if (this.vmSort.key !== key) {
                this.vmSort = { key, dir: 'asc' };
            } else if (this.vmSort.dir === 'asc') {
                this.vmSort = { key, dir: 'desc' };
            } else {
                this.vmSort = { key: 'vmid', dir: 'asc' };  // reset
            }
        },

        sortIndicator(key) {
            if (this.vmSort.key !== key) return '';
            return this.vmSort.dir === 'asc' ? ' ▲' : ' ▼';
        },

        clearVmFilters() {
            this.vmFilter = { search: '', status: '', cluster: '', node: '' };
        },

        // ----- UI helpers -----

        openVm(vm) {
            this.selectedVm = vm;
            this.route = 'vm-detail';
            location.hash = `#/vm/${vm.cluster}/${vm.vmid}`;
        },

        relativeTime(ts) {
            if (!ts) return '';
            const diff = Date.now() - ts;
            if (diff < 2000) return 'just now';
            if (diff < 60000) return Math.floor(diff / 1000) + 's ago';
            if (diff < 3600000) return Math.floor(diff / 60000) + 'm ago';
            return Math.floor(diff / 3600000) + 'h ago';
        },

        humanBytes(n) {
            if (n == null) return '—';
            const units = ['B', 'KB', 'MB', 'GB', 'TB'];
            let i = 0;
            while (n >= 1024 && i < units.length - 1) { n /= 1024; i++; }
            return n.toFixed(i === 0 ? 0 : 1) + ' ' + units[i];
        },

        pct(n) {
            if (n == null) return '—';
            return (n * 100).toFixed(1) + '%';
        },

        uptimeHuman(secs) {
            if (secs == null) return '—';
            const d = Math.floor(secs / 86400);
            const h = Math.floor((secs % 86400) / 3600);
            const m = Math.floor((secs % 3600) / 60);
            if (d) return `${d}d ${h}h`;
            if (h) return `${h}h ${m}m`;
            return `${m}m`;
        },
    };
}