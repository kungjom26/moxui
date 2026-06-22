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
        // --- i18n / locale ---
        locale: localStorage.getItem('moxui.locale') || 'en',
        translations: {},            // loaded locale key->value map
        localeLoading: false,
        localeError: null,

        // $t() magic method for Alpine.js templates
        $t(key) {
            if (this.translations && this.translations[key]) {
                return this.translations[key];
            }
            // Fallback: English
            if (window.__moxuiFallback && window.__moxuiFallback[key]) {
                return window.__moxuiFallback[key];
            }
            return key;  // show the key as fallback
        },

        async loadLocale(lang) {
            this.locale = lang;
            localStorage.setItem('moxui.locale', lang);
            this.localeLoading = true;
            try {
                const resp = await fetch(`/locales/${lang}.json`);
                if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
                this.translations = await resp.json();
                // Also load fallback English if not already loaded
                if (lang !== 'en' && !window.__moxuiFallback) {
                    const engResp = await fetch('/locales/en.json');
                    if (engResp.ok) {
                        window.__moxuiFallback = await engResp.json();
                    }
                }
                document.documentElement.lang = lang === 'th' ? 'th' : 'en';
                this.localeError = null;
            } catch (e) {
                this.localeError = e.message || String(e);
                // Fall back to English inline
                this.translations = {};
            } finally {
                this.localeLoading = false;
            }
        },

        async setLocale(lang) {
            await this.loadLocale(lang);
            // Re-render UI elements that depend on locale
            // Alpine will auto-rebind $t() calls
        },
        // --- auth ---
        token: localStorage.getItem('moxui.token') || null,
        user: null,
        loginForm: { username: '', password: '' },
        loginError: null,
        loggingIn: false,
        // --- passkey ---
        passkeyBusy: false,
        passkeyError: null,
        passkeySuccess: null,

        // --- theme ---
        theme: localStorage.getItem('moxui.theme') || 'light',

        // --- routing ---
        route: this.parseRoute(),

        // --- Global search ---
        showSearch: false,
        searchQuery: '',
        searchHighlightIdx: 0,

        // --- Notifications ---
        showNotifications: false,
        notifications: [],
        notificationsUnread: 0,
        notificationsPollHandle: null,
        notificationsLastId: null,

        // --- VM creation wizard ---
        vmCreateStep: 0,
        vmCreateSubmitting: false,
        vmCreateError: null,
        vmCreateForm: {
            name: '', vmid: '', cluster: '', node: '', os: 'other',
            cores: 2, sockets: 1, memory: 2048,
            disk_size: 32, storage_pool: '',
            bridge: 'vmbr0', model: 'virtio',
        },
        vmCreateSteps: [
            { label: 'General' },
            { label: 'System' },
            { label: 'Storage' },
            { label: 'Network' },
            { label: 'Summary' },
        ],

        // --- API Keys ---
        apiKeys: null,
        apiKeysError: null,
        showCreateApiKey: false,
        apiKeyForm: { name: '' },
        apiKeySaving: false,
        apiKeyFormError: null,
        newlyCreatedKey: null,
        apiKeyCopied: false,
        apiKeyRevoking: false,

        // --- Shortcuts help ---
        showShortcutsHelp: false,

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

        // --- HA Groups state (Phase 4) ---
        haGroups: null,            // HaGroupRow[] from /api/v1/hagroups
        haGroupsError: null,
        showHaGroupForm: false,
        editingHaGroup: null,      // HaGroupRow being edited, or null for new
        haGroupForm: { group: '', cluster: '', nodes: '', comment: '', nofailback: false, restricted: false },
        haGroupSaving: false,
        haGroupFormError: null,

        // --- Bulk operations state (Phase 4) ---
        selectedVmIds: new Set(),  // Set of "cluster:vmid" strings
        bulkBusy: false,
        bulkProgress: '',
        bulkResults: null,         // { action, results: [{ cluster, node, vmid, upid, error }] }

        // --- Migration state (Phase 4) ---
        showMigrateModal: false,
        migrateForm: { target: '', online: true },
        migrateBusy: false,
        migrateError: null,
        availableNodes: [],        // cached node list for the current cluster

        // ----- Custom Dashboard state (Phase 4) -----
        customDashboard: null,     // CustomDashboardConfig from /api/v1/dashboard/custom
        customDashboardError: null,
        customDashboardLoading: false,
        widgetTypes: [],           // available widget types from the server
        editingWidget: null,       // WidgetConfig being edited in the inline editor
        showWidgetEditor: false,
        dashboardSaving: false,
        dashboardSaved: false,

        // ----- Setup Wizard state -----
        showWizard: false,
        wizardStep: 0,
        wizardApplying: false,
        wizardApplyError: null,
        wizardProxmoxTesting: false,
        wizardProxmoxStatus: null, // null | 'ok' | 'err'
        wizardProxmoxError: null,
        wizardChecks: [],          // [{ label, pass, detail }]
        wizardForm: {
            proxmox_host: '',
            proxmox_port: 8006,
            proxmox_user: 'root@pam',
            proxmox_password: '',
            proxmox_verify_tls: true,
            admin_username: 'admin',
            admin_password: '',
            admin_password_confirm: '',
            admin_email: '',
            feature_2fa: true,
            feature_oidc: false,
            feature_webauthn: true,
            feature_webhooks: false,
        },
        wizardSteps: [
            { label: 'Welcome' },
            { label: 'Proxmox' },
            { label: 'Admin User' },
            { label: 'Features' },
            { label: 'Summary' },
            { label: 'Done' },
        ],

        // ----- lifecycle -----

        async init() {
            // Re-apply theme on every page load (the :class binding reads
            // `theme` but we also set <html> directly so the CSS variables
            // resolve before the first paint).
            document.documentElement.classList.toggle('dark', this.theme === 'dark');

            // Load locale — auto-detect browser language, fall back to stored preference
            const browserLang = navigator.language?.startsWith('th') ? 'th' : 'en';
            const storedLocale = localStorage.getItem('moxui.locale');
            const initialLocale = storedLocale || browserLang;
            // Load English fallback immediately
            try {
                const engResp = await fetch('/locales/en.json');
                if (engResp.ok) {
                    window.__moxuiFallback = await engResp.json();
                }
            } catch (_) {}
            // Then load the user's preferred locale
            await this.loadLocale(initialLocale);

            if (this.token) {
                try {
                    await this.fetchMe();
                    await this.fetchAll();
                } catch (e) {
                    // Token expired or invalid.
                    this.logout();
                }
            }

            // Check if first-run setup wizard should be shown
            if (!this.token) {
                await this.checkSetupWizard();
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

            // Start notification polling when logged in
            if (this.token) {
                this.startNotificationPolling();
            }

            // Keyboard shortcuts
            let prefix = null;
            document.addEventListener('keydown', (e) => {
                if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA' || e.target.tagName === 'SELECT') {
                    // Allow '/' for search even in inputs
                    if (e.key === '/' && !e.metaKey && !e.ctrlKey && !e.altKey) {
                        if (this.showSearch) { this.closeSearch(); return; }
                        this.openSearch();
                        e.preventDefault();
                    }
                    return;
                }
                // 'g' prefix navigation
                if (prefix && !e.metaKey && !e.ctrlKey && !e.altKey) {
                    const map = { d: 'dashboard', v: 'vms', s: 'storages', n: 'networks', h: 'hagroups', a: 'audit', l: 'lxcs' };
                    if (map[e.key]) { location.hash = '#/' + map[e.key]; prefix = null; e.preventDefault(); return; }
                }
                if (e.key === 'g') { prefix = 'g'; setTimeout(() => { prefix = null; }, 800); e.preventDefault(); return; }
                // '/' or Ctrl+K / Cmd+K to open search
                if (e.key === '/' && (!e.metaKey && !e.ctrlKey && !e.altKey)) {
                    e.preventDefault();
                    this.openSearch();
                    return;
                }
                if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
                    e.preventDefault();
                    this.openSearch();
                    return;
                }
                // '?' to show shortcuts help
                if (e.key === '?' && !e.metaKey && !e.ctrlKey && !e.altKey) {
                    e.preventDefault();
                    this.showShortcutsHelp = !this.showShortcutsHelp;
                    return;
                }
                // Esc to close modals/dropdowns
                if (e.key === 'Escape') {
                    if (this.showSearch) { this.closeSearch(); return; }
                    if (this.showShortcutsHelp) { this.showShortcutsHelp = false; return; }
                    if (this.showNotifications) { this.showNotifications = false; return; }
                    if (this.showCreateApiKey) { this.showCreateApiKey = false; return; }
                    if (this.route === 'vm-create') { this.cancelVmCreate(); return; }
                    if (this.vmConfirm) { this.cancelVmAction(); return; }
                    if (this.showMigrateModal) { this.showMigrateModal = false; return; }
                }
            });

            // Listen for install prompt events from the service worker script
            document.addEventListener('sw-install-ready', (e) => {
                // No-op — we handle install via external script
            });
        },

        parseRoute() {
            const hash = location.hash.replace(/^#\/?/, '');
            if (hash.startsWith('vm/')) return 'vm-detail';
            if (hash === 'vm-create') return 'vm-create';
            if (hash === 'apikeys') return 'apikeys';
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
                case 'hagroups':
                    this.stopVmPolling();
                    this.stopVmDetailPolling();
                    this.fetchHaGroups();
                    break;
                case 'dashboard':
                    this.stopVmPolling();
                    this.stopVmDetailPolling();
                    this.fetchCustomDashboard();
                    this.fetchWidgetTypes();
                    break;
                case 'audit':
                    this.stopVmPolling();
                    this.stopVmDetailPolling();
                    this.fetchAudit();
                    break;
                case 'apikeys':
                    this.stopVmPolling();
                    this.stopVmDetailPolling();
                    this.fetchApiKeys();
                    break;
                case 'vm-create':
                    // No data fetching needed; just render the wizard
                    this.stopVmPolling();
                    this.stopVmDetailPolling();
                    this.resetVmCreateForm();
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

        // ----- WebAuthn / Passkey -----

        // Decode a base64url-encoded string to an ArrayBuffer.
        _base64urlToBuffer(b64) {
            const binary = atob(b64.replace(/-/g, '+').replace(/_/g, '/'));
            const bytes = new Uint8Array(binary.length);
            for (let i = 0; i < binary.length; i++) {
                bytes[i] = binary.charCodeAt(i);
            }
            return bytes.buffer;
        },

        // Encode an ArrayBuffer to a base64url string.
        _bufferToBase64url(buf) {
            const bytes = new Uint8Array(buf);
            let binary = '';
            for (let i = 0; i < bytes.byteLength; i++) {
                binary += String.fromCharCode(bytes[i]);
            }
            return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
        },

        // Recursively walk the challenge object and decode all `BufferSource` fields
        // (which arrive as base64url strings from JSON) into ArrayBuffers, so the
        // browser's `navigator.credentials.create()` / `get()` can consume them.
        _decodePublicKey(obj) {
            if (obj == null) return obj;
            if (typeof obj === 'string') return obj;
            if (obj instanceof Array) return obj.map(v => this._decodePublicKey(v));
            if (typeof obj === 'object') {
                const decoded = {};
                for (const [k, v] of Object.entries(obj)) {
                    // Fields that are base64url in JSON become ArrayBuffer for the WebAuthn API.
                    if ((k === 'challenge' || k === 'id' || k.endsWith('Id')) && typeof v === 'string') {
                        decoded[k] = this._base64urlToBuffer(v);
                    } else if (k === 'transports' && Array.isArray(v)) {
                        decoded[k] = v;  // transports is an array of strings, keep as-is
                    } else if (k === 'allowCredentials' && Array.isArray(v)) {
                        decoded[k] = v.map(cred => ({
                            ...cred,
                            id: this._base64urlToBuffer(cred.id),
                        }));
                    } else if (k === 'credential' && typeof v === 'object' && v !== null) {
                        // The `credential` field from register/complete input — keep as-is
                        // (it's the browser's PublicKeyCredential which has ArrayBuffers).
                        decoded[k] = v;
                    } else {
                        decoded[k] = this._decodePublicKey(v);
                    }
                }
                return decoded;
            }
            return obj;
        },

        // Encode the browser's PublicKeyCredential response back to a JSON-safe
        // object with base64url-encoded ArrayBuffer fields.
        _encodeCredential(cred) {
            const obj = {
                id: cred.id,
                type: cred.type,
                rawId: this._bufferToBase64url(cred.rawId),
                response: {},
                clientExtensionResults: cred.getClientExtensionResults ? cred.getClientExtensionResults() : {},
                authenticatorAttachment: cred.authenticatorAttachment || null,
            };
            const resp = cred.response;
            if (resp.attestationObject) {
                obj.response.attestationObject = this._bufferToBase64url(resp.attestationObject);
            }
            if (resp.clientDataJSON) {
                obj.response.clientDataJSON = this._bufferToBase64url(resp.clientDataJSON);
            }
            if (resp.authenticatorData) {
                obj.response.authenticatorData = this._bufferToBase64url(resp.authenticatorData);
            }
            if (resp.signature) {
                obj.response.signature = this._bufferToBase64url(resp.signature);
            }
            if (resp.userHandle) {
                obj.response.userHandle = this._bufferToBase64url(resp.userHandle);
            }
            return obj;
        },

        async loginWithPasskey() {
            this.passkeyError = null;
            this.passkeySuccess = null;
            const username = this.loginForm.username.trim();
            if (!username) {
                this.passkeyError = 'Enter your username first, then click Login with Passkey.';
                return;
            }
            this.passkeyBusy = true;
            try {
                // 1. Get assertion challenge from server
                const startResp = await fetch('/api/v1/auth/webauthn/login/start', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ username }),
                });
                if (!startResp.ok) {
                    const err = await startResp.json().catch(() => ({}));
                    throw new Error(err.message || err.error || `Passkey login start failed (${startResp.status})`);
                }
                const { challenge } = await startResp.json();

                // 2. Decode challenge for the browser WebAuthn API
                const publicKey = this._decodePublicKey(challenge);

                // 3. Ask browser for the passkey assertion
                const credential = await navigator.credentials.get({ publicKey });

                // 4. Encode the credential back to JSON-safe format
                const encoded = this._encodeCredential(credential);

                // 5. Send to server for verification
                const completeResp = await fetch('/api/v1/auth/webauthn/login/complete', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ username, credential: encoded }),
                });
                if (!completeResp.ok) {
                    const err = await completeResp.json().catch(() => ({}));
                    throw new Error(err.message || err.error || `Passkey login failed (${completeResp.status})`);
                }
                const data = await completeResp.json();

                // 6. Store JWT and proceed as normal login
                this.token = data.token;
                localStorage.setItem('moxui.token', data.token);
                this.loginForm = { username: '', password: '' };
                await this.fetchMe();
                await this.fetchAll();
                this.route = 'vms';
                location.hash = '#/vms';
            } catch (e) {
                this.passkeyError = e.message || String(e);
            } finally {
                this.passkeyBusy = false;
            }
        },

        async registerPasskey() {
            this.passkeyError = null;
            this.passkeySuccess = null;
            this.passkeyBusy = true;
            try {
                // 1. Get creation challenge from server
                const startResp = await this.api('/api/v1/auth/webauthn/register/start', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({}),
                });
                const { challenge } = await startResp.json();

                // 2. Decode challenge for the browser WebAuthn API
                const publicKey = this._decodePublicKey(challenge);

                // 3. Ask browser to create a passkey
                const credential = await navigator.credentials.create({ publicKey });

                // 4. Encode the credential for JSON
                const encoded = this._encodeCredential(credential);

                // 5. Send to server for verification
                const completeResp = await this.api('/api/v1/auth/webauthn/register/complete', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ credential: encoded }),
                });
                await completeResp.json();

                this.passkeySuccess = 'Passkey registered successfully!';
                setTimeout(() => { this.passkeySuccess = null; }, 5000);
            } catch (e) {
                this.passkeyError = e.message || String(e);
                setTimeout(() => { this.passkeyError = null; }, 5000);
            } finally {
                this.passkeyBusy = false;
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
            this.stopNotificationPolling();
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
        canMigrate()  { return this.selectedVm && this.selectedVm.status === 'running'; },

        // ----- HA Group management (Phase 4) -----

        async fetchHaGroups() {
            try {
                const resp = await this.api('/api/v1/hagroups');
                const body = await resp.json();
                this.haGroups = body.groups || [];
                if (body.errors && Object.keys(body.errors).length > 0) {
                    this.haGroupsError = 'Some clusters failed: ' +
                        Object.entries(body.errors).map(([k, v]) => `${k}: ${v}`).join('; ');
                } else {
                    this.haGroupsError = null;
                }
            } catch (e) {
                this.haGroupsError = e.message || String(e);
            }
        },

        editHaGroup(g) {
            this.editingHaGroup = g;
            this.haGroupForm = {
                group: g.group,
                cluster: g.cluster,
                nodes: g.nodes || '',
                comment: g.comment || '',
                nofailback: g.nofailback == 1,
                restricted: g.restricted == 1,
            };
            this.showHaGroupForm = true;
            this.haGroupFormError = null;
        },

        cancelHaGroupForm() {
            this.showHaGroupForm = false;
            this.editingHaGroup = null;
            this.haGroupForm = { group: '', cluster: '', nodes: '', comment: '', nofailback: false, restricted: false };
            this.haGroupFormError = null;
        },

        async saveHaGroup() {
            this.haGroupSaving = true;
            this.haGroupFormError = null;
            const { group, cluster, nodes, comment, nofailback, restricted } = this.haGroupForm;
            try {
                const body = { nodes: nodes || undefined, comment: comment || undefined, nofailback, restricted };
                await this.api(`/api/v1/hagroups/${cluster}/${group}`, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(body),
                });
                this.cancelHaGroupForm();
                await this.fetchHaGroups();
            } catch (e) {
                this.haGroupFormError = e.message || String(e);
            } finally {
                this.haGroupSaving = false;
            }
        },

        async deleteHaGroup(g) {
            if (!confirm(`Delete HA group "${g.group}" on cluster "${g.cluster}"?`)) return;
            try {
                await this.api(`/api/v1/hagroups/${g.cluster}/${g.group}`, { method: 'DELETE' });
                await this.fetchHaGroups();
            } catch (e) {
                this.haGroupsError = e.message || String(e);
            }
        },

        // ----- Bulk operations (Phase 4) -----

        toggleVmSelection(vm) {
            const key = vm.cluster + ':' + vm.vmid;
            if (this.selectedVmIds.has(key)) {
                this.selectedVmIds.delete(key);
            } else {
                this.selectedVmIds.add(key);
            }
            // Force reactivity: Alpine needs a fresh reference for Sets.
            this.selectedVmIds = new Set(this.selectedVmIds);
        },

        toggleSelectAll() {
            const filtered = this.filteredVms;
            if (this.allVmsSelected) {
                // Deselect all visible.
                for (const vm of filtered) {
                    this.selectedVmIds.delete(vm.cluster + ':' + vm.vmid);
                }
            } else {
                // Select all visible.
                for (const vm of filtered) {
                    this.selectedVmIds.add(vm.cluster + ':' + vm.vmid);
                }
            }
            this.selectedVmIds = new Set(this.selectedVmIds);
        },

        get allVmsSelected() {
            const filtered = this.filteredVms;
            if (!filtered || filtered.length === 0) return false;
            return filtered.every(vm => this.selectedVmIds.has(vm.cluster + ':' + vm.vmid));
        },

        clearVmSelection() {
            this.selectedVmIds = new Set();
            this.bulkResults = null;
        },

        async bulkAction(action) {
            this.bulkBusy = true;
            this.bulkResults = null;
            this.bulkProgress = '';
            // Build list of VM refs from selected IDs.
            const vms = [];
            for (const key of this.selectedVmIds) {
                const [cluster, vmidStr] = key.split(':');
                const vmid = Number(vmidStr);
                const vm = (this.vms || []).find(v => v.cluster === cluster && v.vmid === vmid);
                if (vm) {
                    vms.push({ cluster, node: vm.node, vmid });
                }
            }
            this.bulkProgress = `Sending ${action} for ${vms.length} VM(s)...`;
            try {
                const resp = await this.api(`/api/v1/vms/bulk/${action}`, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ vms }),
                });
                this.bulkResults = await resp.json();
                // Clear selection after successful bulk action.
                this.selectedVmIds = new Set();
                // Refresh VM list to reflect status changes.
                this.fetchVms();
            } catch (e) {
                this.bulkResults = { action, results: [{ error: e.message || String(e) }] };
            } finally {
                this.bulkBusy = false;
                this.bulkProgress = '';
                // Auto-hide results after 10 seconds.
                if (this.bulkResults) {
                    setTimeout(() => { this.bulkResults = null; }, 10000);
                }
            }
        },

        // ----- Live Migration (Phase 4) -----

        async loadNodes() {
            // Build a deduplicated list of nodes from the current cluster's VMs.
            const cluster = this.selectedVm?.cluster;
            if (!cluster || !this.vms) return;
            const nodes = new Set();
            for (const vm of this.vms) {
                if (vm.cluster === cluster && vm.node !== this.selectedVm?.node) {
                    nodes.add(vm.node);
                }
            }
            this.availableNodes = [...nodes].sort();
        },

        async confirmMigrate() {
            if (!this.migrateForm.target) return;
            this.migrateBusy = true;
            this.migrateError = null;
            const { cluster, node, vmid } = this.selectedVm;
            const { target, online } = this.migrateForm;
            try {
                const resp = await this.api(`/api/v1/vms/${cluster}/${node}/${vmid}/migrate`, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ target, online }),
                });
                const data = await resp.json();
                this.showMigrateModal = false;
                if (data.upid) {
                    this.vmActionPending = { upid: data.upid, action: 'migrate', startedAt: Date.now() };
                    this.fetchVmDetail();
                    this.fetchVmTask();
                }
            } catch (e) {
                this.migrateError = e.message || String(e);
            } finally {
                this.migrateBusy = false;
            }
        },

        // ----- Custom Dashboard (Phase 4) -----

        async fetchCustomDashboard() {
            this.customDashboardLoading = true;
            this.customDashboardError = null;
            try {
                const resp = await this.api('/api/v1/dashboard/custom');
                this.customDashboard = await resp.json();
            } catch (e) {
                this.customDashboardError = e.message || String(e);
            } finally {
                this.customDashboardLoading = false;
            }
        },

        async fetchWidgetTypes() {
            try {
                const resp = await this.api('/api/v1/dashboard/custom/widget-types');
                this.widgetTypes = await resp.json();
            } catch (e) {
                console.warn('Failed to load widget types:', e);
            }
        },

        addWidget(type) {
            if (!this.customDashboard || !this.widgetTypes) return;
            const template = this.widgetTypes.find(t => t.type === type);
            if (!template) return;
            const id = 'widget-' + Date.now();
            const maxY = this.customDashboard.widgets.reduce((max, w) => Math.max(max, w.y + w.height), 1);
            const widget = {
                id,
                type: template.type,
                title: template.label,
                x: 1,
                y: maxY,
                width: template.default_width || 6,
                height: template.default_height || 2,
            };
            this.customDashboard.widgets.push(widget);
            this.customDashboard.layout.push({
                row: maxY,
                widgets: [id],
            });
            this.saveCustomDashboard();
        },

        removeWidget(id) {
            if (!this.customDashboard) return;
            this.customDashboard.widgets = this.customDashboard.widgets.filter(w => w.id !== id);
            this.customDashboard.layout = this.customDashboard.layout.filter(r =>
                r.widgets = r.widgets.filter(w => w !== id)
            ).filter(r => r.widgets.length > 0);
            if (this.editingWidget && this.editingWidget.id === id) {
                this.editingWidget = null;
                this.showWidgetEditor = false;
            }
            this.saveCustomDashboard();
        },

        editWidget(widget) {
            this.editingWidget = { ...widget };
            this.showWidgetEditor = true;
        },

        saveWidgetEdit() {
            if (!this.editingWidget || !this.customDashboard) return;
            const idx = this.customDashboard.widgets.findIndex(w => w.id === this.editingWidget.id);
            if (idx >= 0) {
                this.customDashboard.widgets[idx] = { ...this.editingWidget };
            }
            this.editingWidget = null;
            this.showWidgetEditor = false;
            this.saveCustomDashboard();
        },

        cancelWidgetEdit() {
            this.editingWidget = null;
            this.showWidgetEditor = false;
        },

        async saveCustomDashboard() {
            if (!this.customDashboard) return;
            this.dashboardSaving = true;
            this.dashboardSaved = false;
            try {
                await this.api('/api/v1/dashboard/custom', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ dashboard: this.customDashboard }),
                });
                this.dashboardSaved = true;
                setTimeout(() => { this.dashboardSaved = false; }, 3000);
            } catch (e) {
                this.customDashboardError = e.message || String(e);
            } finally {
                this.dashboardSaving = false;
            }
        },

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

        // ----- Setup Wizard -----

        get wizardNextDisabled() {
            switch (this.wizardStep) {
                case 0: return false;
                case 1: return !this.wizardForm.proxmox_host || !this.wizardForm.proxmox_port || !this.wizardForm.proxmox_user || !this.wizardForm.proxmox_password;
                case 2: return !this.wizardForm.admin_username || !this.wizardForm.admin_password || this.wizardForm.admin_password !== this.wizardForm.admin_password_confirm;
                case 3: return false;
                default: return false;
            }
        },

        async checkSetupWizard() {
            try {
                const resp = await fetch('/api/v1/setup/status');
                if (!resp.ok) return;
                const data = await resp.json();
                // Show wizard when no Proxmox cluster is configured
                if (data.needs_setup) {
                    this.showWizard = true;
                    this.runSystemChecks();
                }
            } catch (_) {
                // Backend doesn't have setup endpoint yet — hide wizard
                this.showWizard = false;
            }
        },

        async runSystemChecks() {
            this.wizardChecks = [
                { label: 'Browser support', pass: true, detail: 'Modern browser detected' },
                { label: 'LocalStorage', pass: typeof localStorage !== 'undefined', detail: '' },
                { label: 'Fetch API', pass: typeof fetch !== 'undefined', detail: '' },
                { label: 'WebAuthn', pass: typeof navigator?.credentials?.create === 'function', detail: '' },
                { label: 'WebSocket', pass: typeof WebSocket !== 'undefined', detail: '' },
                { label: 'Crypto Subtle', pass: typeof crypto?.subtle !== 'undefined', detail: '' },
            ];
        },

        nextWizardStep() {
            if (this.wizardNextDisabled) return;
            if (this.wizardStep < this.wizardSteps.length - 1) {
                this.wizardStep++;
            }
        },

        prevWizardStep() {
            if (this.wizardStep > 0) {
                this.wizardStep--;
            }
        },

        async testProxmoxConnection() {
            this.wizardProxmoxTesting = true;
            this.wizardProxmoxStatus = null;
            this.wizardProxmoxError = null;
            try {
                const resp = await fetch('/api/v1/setup/test-proxmox', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({
                        host: this.wizardForm.proxmox_host,
                        port: this.wizardForm.proxmox_port,
                        user: this.wizardForm.proxmox_user,
                        password: this.wizardForm.proxmox_password,
                        verify_tls: this.wizardForm.proxmox_verify_tls,
                    }),
                });
                const data = await resp.json();
                if (resp.ok && data.ok) {
                    this.wizardProxmoxStatus = 'ok';
                } else {
                    this.wizardProxmoxStatus = 'err';
                    this.wizardProxmoxError = data.error || data.message || 'Connection failed';
                }
            } catch (e) {
                this.wizardProxmoxStatus = 'err';
                this.wizardProxmoxError = e.message || String(e);
            } finally {
                this.wizardProxmoxTesting = false;
            }
        },

        async applyWizardConfig() {
            this.wizardApplying = true;
            this.wizardApplyError = null;
            try {
                const payload = {
                    proxmox: {
                        host: this.wizardForm.proxmox_host,
                        port: this.wizardForm.proxmox_port,
                        user: this.wizardForm.proxmox_user,
                        password: this.wizardForm.proxmox_password,
                        verify_tls: this.wizardForm.proxmox_verify_tls,
                    },
                    admin: {
                        username: this.wizardForm.admin_username,
                        password: this.wizardForm.admin_password,
                        email: this.wizardForm.admin_email || undefined,
                    },
                    features: {
                        totp_2fa: this.wizardForm.feature_2fa,
                        oidc: this.wizardForm.feature_oidc,
                        webauthn: this.wizardForm.feature_webauthn,
                        webhooks: this.wizardForm.feature_webhooks,
                    },
                };

                const resp = await fetch('/api/v1/setup/apply', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(payload),
                });

                if (!resp.ok) {
                    const err = await resp.json().catch(() => ({}));
                    throw new Error(err.message || err.error || `Setup failed (${resp.status})`);
                }

                // Move to the "Done" step
                this.wizardStep = 5;

                // Auto-login with the new admin credentials and redirect to dashboard
                setTimeout(() => {
                    this.loginForm = {
                        username: this.wizardForm.admin_username,
                        password: this.wizardForm.admin_password,
                    };
                    this.showWizard = false;
                    this.login();
                }, 2000);
            } catch (e) {
                this.wizardApplyError = e.message || String(e);
            } finally {
                this.wizardApplying = false;
            }
        },

        // ----- Global Search -----

        openSearch() {
            this.showSearch = true;
            this.searchQuery = '';
            this.searchHighlightIdx = 0;
            this.$nextTick(() => {
                if (this.$refs.searchInput) {
                    this.$refs.searchInput.focus();
                }
            });
        },

        closeSearch() {
            this.showSearch = false;
            this.searchQuery = '';
            this.searchHighlightIdx = 0;
        },

        get searchResults() {
            const q = (this.searchQuery || '').trim().toLowerCase();
            if (!q) return { vms: [], lxcs: [], storage: [], nodes: [], clusters: [] };
            const out = { vms: [], lxcs: [], storage: [], nodes: [], clusters: [] };

            // Search VMs
            if (this.vms) {
                out.vms = this.vms.filter(vm => {
                    const hay = [String(vm.vmid), (vm.name || '').toLowerCase(), vm.node.toLowerCase(), vm.cluster.toLowerCase(), (vm.tags || '').toLowerCase()].join(' ');
                    return hay.includes(q);
                }).slice(0, 10);
            }

            // Search LXCs
            if (this.lxcs) {
                out.lxcs = this.lxcs.filter(c => {
                    const hay = [String(c.vmid), (c.name || '').toLowerCase(), c.node.toLowerCase(), c.cluster.toLowerCase()].join(' ');
                    return hay.includes(q);
                }).slice(0, 5);
            }

            // Search Storage
            if (this.storages) {
                out.storage = this.storages.filter(s => {
                    const hay = [s.storage.toLowerCase(), (s.kind || '').toLowerCase(), s.cluster.toLowerCase()].join(' ');
                    return hay.includes(q);
                }).slice(0, 5);
            }

            // Search Nodes
            if (this.vms) {
                const nodeSet = new Set();
                this.vms.forEach(v => {
                    if (v.node.toLowerCase().includes(q)) nodeSet.add(v.node);
                    if (v.cluster.toLowerCase().includes(q)) {
                        if (!out.clusters.includes(v.cluster)) out.clusters.push(v.cluster);
                    }
                });
                out.nodes = [...nodeSet].slice(0, 5);
            }

            return out;
        },

        searchHighlightNext() {
            const total = this.searchResults.vms.length + this.searchResults.lxcs.length + this.searchResults.storage.length;
            if (this.searchHighlightIdx < total - 1) this.searchHighlightIdx++;
        },

        searchHighlightPrev() {
            if (this.searchHighlightIdx > 0) this.searchHighlightIdx--;
        },

        searchSelectHighlighted() {
            const { vms, lxcs, storage } = this.searchResults;
            const totalVm = vms.length;
            const totalLxc = totalVm + lxcs.length;

            if (this.searchHighlightIdx < totalVm) {
                this.searchNavigate(vms[this.searchHighlightIdx]);
            } else if (this.searchHighlightIdx < totalLxc) {
                this.searchNavigate(lxcs[this.searchHighlightIdx - totalVm], 'lxc');
            } else if (this.searchHighlightIdx < totalLxc + storage.length) {
                this.searchNavigateToRoute('storages');
            }
        },

        searchNavigate(item, type) {
            if (type === 'lxc') {
                location.hash = '#/lxcs';
                // Scroll to the LXC — we just go to the list view
            } else if (item && item.cluster && item.node && item.vmid) {
                this.openVm(item);
            }
            this.closeSearch();
        },

        searchNavigateToRoute(route) {
            location.hash = '#/' + route;
            this.closeSearch();
        },

        // ----- VM Creation Wizard -----

        openVmCreate() {
            this.route = 'vm-create';
            location.hash = '#/vm-create';
        },

        cancelVmCreate() {
            this.route = 'vms';
            location.hash = '#/vms';
            this.vmCreateStep = 0;
            this.vmCreateError = null;
            this.vmCreateSubmitting = false;
        },

        resetVmCreateForm() {
            this.vmCreateStep = 0;
            this.vmCreateError = null;
            this.vmCreateSubmitting = false;
            this.vmCreateForm = {
                name: '', vmid: '', cluster: '', node: '', os: 'other',
                cores: 2, sockets: 1, memory: 2048,
                disk_size: 32, storage_pool: '',
                bridge: 'vmbr0', model: 'virtio',
            };
        },

        get vmCreateNextDisabled() {
            switch (this.vmCreateStep) {
                case 0: return !this.vmCreateForm.name || !this.vmCreateForm.cluster || !this.vmCreateForm.node;
                case 1: return !this.vmCreateForm.cores || !this.vmCreateForm.memory;
                case 2: return !this.vmCreateForm.disk_size || !this.vmCreateForm.storage_pool;
                case 3: return false;
                default: return false;
            }
        },

        nextVmCreateStep() {
            if (this.vmCreateNextDisabled) return;
            if (this.vmCreateStep < 4) this.vmCreateStep++;
        },

        prevVmCreateStep() {
            if (this.vmCreateStep > 0) this.vmCreateStep--;
        },

        async submitVmCreate() {
            this.vmCreateSubmitting = true;
            this.vmCreateError = null;
            try {
                const body = {
                    name: this.vmCreateForm.name,
                    vmid: this.vmCreateForm.vmid ? Number(this.vmCreateForm.vmid) : undefined,
                    cluster: this.vmCreateForm.cluster,
                    node: this.vmCreateForm.node,
                    os: this.vmCreateForm.os,
                    cores: Number(this.vmCreateForm.cores),
                    sockets: Number(this.vmCreateForm.sockets),
                    memory: Number(this.vmCreateForm.memory),
                    disk_size: Number(this.vmCreateForm.disk_size),
                    storage_pool: this.vmCreateForm.storage_pool,
                    bridge: this.vmCreateForm.bridge,
                    model: this.vmCreateForm.model,
                };
                await this.api('/api/v1/vms', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(body),
                });
                this.addNotification({ icon: '✅', message: this.$t('wizard.success'), type: 'vm_created' });
                this.cancelVmCreate();
                this.fetchVms();
            } catch (e) {
                this.vmCreateError = e.message || String(e);
            } finally {
                this.vmCreateSubmitting = false;
            }
        },

        // ----- Notifications -----

        startNotificationPolling() {
            this.stopNotificationPolling();
            this.fetchNotifications();
            this.notificationsPollHandle = setInterval(() => this.fetchNotifications(), 10000);
        },

        stopNotificationPolling() {
            if (this.notificationsPollHandle) {
                clearInterval(this.notificationsPollHandle);
                this.notificationsPollHandle = null;
            }
        },

        async fetchNotifications() {
            if (!this.token) return;
            try {
                const resp = await this.api('/api/v1/notifications');
                const data = await resp.json();
                if (data.notifications) {
                    this.notifications = data.notifications;
                    this.notificationsUnread = this.notifications.filter(n => !n.read).length;
                }
            } catch (_) {
                // Notifications endpoint may not exist yet — silently ignore
                // Seed with demo notifications so the UI is testable
                if (this.notifications.length === 0) {
                    // Don't seed — just show empty state
                }
            }
        },

        toggleNotifications() {
            this.showNotifications = !this.showNotifications;
            if (this.showNotifications) {
                this.fetchNotifications();
            }
        },

        markNotificationRead(n, idx) {
            if (n.read) return;
            // Optimistic update
            n.read = true;
            this.notificationsUnread = this.notifications.filter(nn => !nn.read).length;
            // Attempt server-side mark
            if (n.id) {
                this.api(`/api/v1/notifications/${n.id}/read`, { method: 'POST' }).catch(() => {});
            }
        },

        markAllNotificationsRead() {
            this.notifications.forEach(n => n.read = true);
            this.notificationsUnread = 0;
            this.api('/api/v1/notifications/read-all', { method: 'POST' }).catch(() => {});
        },

        addNotification(n) {
            this.notifications.unshift({ id: Date.now(), read: false, time: Date.now(), ...n });
            this.notificationsUnread = this.notifications.filter(nn => !nn.read).length;
        },

        // ----- CSV Export -----

        exportCsv(data, columns, filename) {
            if (!data || data.length === 0) return;
            const header = columns.map(c => {
                if (typeof c === 'string') return c;
                return c.label || c.key || '';
            }).join(',');
            const rows = data.map(row => {
                return columns.map(c => {
                    const key = typeof c === 'string' ? c : c.key;
                    let val = row[key];
                    if (val == null) val = '';
                    // Escape quotes and wrap in quotes if contains comma
                    val = String(val).replace(/"/g, '""');
                    if (val.includes(',') || val.includes('"') || val.includes('\n')) {
                        val = '"' + val + '"';
                    }
                    return val;
                }).join(',');
            }).join('\n');
            const csv = header + '\n' + rows;
            const blob = new Blob([csv], { type: 'text/csv;charset=utf-8;' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = filename || 'export.csv';
            a.click();
            URL.revokeObjectURL(url);
        },

        exportVmStatsCsv() {
            if (!this.selectedVm) return;
            const cols = [
                { key: 'vmid', label: 'VMID' },
                { key: 'name', label: 'Name' },
                { key: 'cluster', label: 'Cluster' },
                { key: 'node', label: 'Node' },
                { key: 'status', label: 'Status' },
                { key: 'cpu', label: 'CPU %' },
                { key: 'mem', label: 'Memory' },
                { key: 'maxmem', label: 'Max Memory' },
                { key: 'uptime', label: 'Uptime (s)' },
            ];
            const svm = this.selectedVm;
            const row = {
                vmid: svm.vmid,
                name: svm.name || '',
                cluster: svm.cluster,
                node: svm.node,
                status: svm.status,
                cpu: svm.cpu != null ? (svm.cpu * 100).toFixed(1) : '',
                mem: svm.mem || '',
                maxmem: svm.maxmem || '',
                uptime: svm.uptime || '',
            };
            const csv = cols.map(c => c.label).join(',') + '\n' +
                cols.map(c => {
                    let v = String(row[c.key] ?? '');
                    v = v.replace(/"/g, '""');
                    return v.includes(',') ? '"' + v + '"' : v;
                }).join(',');
            const blob = new Blob([csv], { type: 'text/csv;charset=utf-8;' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = `vm-${svm.vmid}-stats.csv`;
            a.click();
            URL.revokeObjectURL(url);
        },

        // ----- API Keys -----

        async fetchApiKeys() {
            this.apiKeysError = null;
            try {
                const resp = await this.api('/api/v1/apikeys');
                const data = await resp.json();
                this.apiKeys = data.keys || [];
            } catch (e) {
                this.apiKeysError = e.message || String(e);
                this.apiKeys = [];
            }
        },

        async saveApiKey() {
            if (!this.apiKeyForm.name) return;
            this.apiKeySaving = true;
            this.apiKeyFormError = null;
            this.newlyCreatedKey = null;
            try {
                const resp = await this.api('/api/v1/apikeys', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ name: this.apiKeyForm.name }),
                });
                const data = await resp.json();
                this.newlyCreatedKey = data.key;
                this.apiKeyCopied = false;
                this.apiKeyForm.name = '';
                await this.fetchApiKeys();
            } catch (e) {
                this.apiKeyFormError = e.message || String(e);
            } finally {
                this.apiKeySaving = false;
            }
        },

        cancelCreateApiKey() {
            this.showCreateApiKey = false;
            this.apiKeyForm = { name: '' };
            this.apiKeyFormError = null;
            this.newlyCreatedKey = null;
            this.apiKeyCopied = false;
        },

        async revokeApiKey(key) {
            if (!confirm(this.$t('apikeys.confirm_revoke'))) return;
            this.apiKeyRevoking = true;
            try {
                await this.api(`/api/v1/apikeys/${key.id}`, { method: 'DELETE' });
                await this.fetchApiKeys();
            } catch (e) {
                this.apiKeysError = e.message || String(e);
            } finally {
                this.apiKeyRevoking = false;
            }
        },

        copyNewApiKey() {
            if (this.newlyCreatedKey) {
                navigator.clipboard.writeText(this.newlyCreatedKey).then(() => {
                    this.apiKeyCopied = true;
                    setTimeout(() => { this.apiKeyCopied = false; }, 3000);
                }).catch(() => {
                    // Fallback: select the text
                    this.apiKeyCopied = true;
                    setTimeout(() => { this.apiKeyCopied = false; }, 3000);
                });
            }
        },
    };
}