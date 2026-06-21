// moxui — Alpine.js SPA logic.
//
// State shape: see the `moxui()` factory below. We persist `token` and
// `theme` to localStorage so a page refresh doesn't kick the user back
// to the login screen. User details are NOT persisted — we re-fetch
// /api/v1/auth/me on every page load to validate the token is still good.
//
// Routes: see the `route` field. Hash-based (`#/vms`, `#/lxcs`, etc.)
// because we don't want to bother the backend with router history. Day
// 11+ will add proper VM detail routes (`#/vms/<cluster>/<vmid>`).

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
        vms: null,
        lxcs: null,
        storages: null,
        networks: null,
        selectedVm: null,

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
            const hash = location.hash.replace(/^#\//, '');
            return hash || 'vms';
        },

        maybeLoadRoute() {
            // Day 11 will reload VM list every 2s while on /vms.
            // For now, just one-shot fetches.
            if (!this.token) return;
            switch (this.route) {
                case 'vms':      this.fetchVms(); break;
                case 'lxcs':     this.fetchLxcs(); break;
                case 'storages': this.fetchStorages(); break;
                case 'networks': this.fetchNetworks(); break;
                case 'vm-detail': break;  // already loaded
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

        async fetchVms()      { this.vms      = (await (await this.api('/api/v1/vms'     )).json()).vms; },
        async fetchLxcs()     { this.lxcs     = (await (await this.api('/api/v1/lxcs'    )).json()).lxcs; },
        async fetchStorages() { this.storages = (await (await this.api('/api/v1/storages')).json()).storages; },
        async fetchNetworks() { this.networks = (await (await this.api('/api/v1/networks')).json()).networks; },

        async api(path) {
            const resp = await fetch(path, {
                headers: { 'Authorization': 'Bearer ' + this.token },
            });
            if (resp.status === 401) { this.logout(); throw new Error('unauthorized'); }
            if (!resp.ok) {
                const err = await resp.json().catch(() => ({}));
                throw new Error(err.message || err.error || `${path} → HTTP ${resp.status}`);
            }
            return resp;
        },

        // ----- UI helpers -----

        openVm(vm) {
            this.selectedVm = vm;
            this.route = 'vm-detail';
            location.hash = `#/vm/${vm.cluster}/${vm.vmid}`;
        },

        humanBytes(n) {
            if (n == null) return '—';
            const units = ['B', 'KB', 'MB', 'GB', 'TB'];
            let i = 0;
            while (n >= 1024 && i < units.length - 1) { n /= 1024; i++; }
            return n.toFixed(i === 0 ? 0 : 1) + ' ' + units[i];
        },
    };
}
