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
        selectedVm: null,

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
                    const map = { v: 'vms', l: 'lxcs', s: 'storages', n: 'networks', d: 'vms' };
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
            // hash like "#/vm/<cluster>/<vmid>" → { cluster, vmid }
            const m = location.hash.match(/^#\/vm\/([^/]+)\/(\d+)/);
            return m ? { cluster: m[1], vmid: Number(m[2]) } : null;
        },

        maybeLoadRoute() {
            if (!this.token) return;
            switch (this.route) {
                case 'vms':
                    this.startVmPolling();
                    this.fetchVms();
                    break;
                case 'lxcs':
                    this.stopVmPolling();
                    this.fetchLxcs();
                    break;
                case 'storages':
                    this.stopVmPolling();
                    this.fetchStorages();
                    break;
                case 'networks':
                    this.stopVmPolling();
                    this.fetchNetworks();
                    break;
                case 'vm-detail': {
                    this.stopVmPolling();
                    const sel = this.parseVmDetail();
                    if (sel) {
                        this.selectedVm = { cluster: sel.cluster, vmid: sel.vmid };
                        // Try to populate from the cached list (we may have it).
                        const cached = (this.vms || []).find(v =>
                            v.cluster === sel.cluster && v.vmid === sel.vmid
                        );
                        if (cached) Object.assign(this.selectedVm, cached);
                    }
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

        logout() {
            this.token = null;
            this.user = null;
            localStorage.removeItem('moxui.token');
            this.vms = this.lxcs = this.storages = this.networks = null;
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