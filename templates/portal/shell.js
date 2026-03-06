(function () {
    const ROOT_ID = "pc-shell-root";
    const ROLE_STORAGE_KEY = "quotey.portal.app_shell_role";

    const ROLE_CONFIG = {
        rep: {
            label: "Rep",
            links: [
                { href: "/portal", label: "Quotes" },
                { href: "/approvals", label: "Approvals" },
            ],
        },
        manager: {
            label: "Manager",
            links: [
                { href: "/approvals", label: "Approval Queue" },
                { href: "/portal?status=pending", label: "Pipeline" },
                { href: "/settings", label: "Settings" },
            ],
        },
        ops: {
            label: "Ops",
            links: [
                { href: "/portal", label: "Dashboard" },
                { href: "/approvals", label: "Approvals" },
                { href: "/settings", label: "Policies" },
            ],
        },
    };

    function normalizeRole(rawRole) {
        const candidate = (rawRole || "").toLowerCase();
        if (Object.prototype.hasOwnProperty.call(ROLE_CONFIG, candidate)) {
            return candidate;
        }
        return "rep";
    }

    function selectRole() {
        const params = new URLSearchParams(window.location.search);
        const roleFromQuery = normalizeRole(params.get("role"));
        if (params.has("role")) {
            localStorage.setItem(ROLE_STORAGE_KEY, roleFromQuery);
            return roleFromQuery;
        }
        return normalizeRole(localStorage.getItem(ROLE_STORAGE_KEY));
    }

    function routeLabel(pathname) {
        if (pathname.startsWith("/quote/")) {
            return "Quote Detail";
        }
        if (pathname.startsWith("/approvals")) {
            return "Approvals";
        }
        if (pathname.startsWith("/settings")) {
            return "Settings";
        }
        return "Portal";
    }

    function linkIsActive(href) {
        const targetUrl = new URL(href, window.location.origin);
        const currentPath = window.location.pathname;
        const targetPath = targetUrl.pathname;
        if (targetPath === "/portal" && currentPath === "/") {
            return true;
        }
        return currentPath === targetPath || currentPath.startsWith(`${targetPath}/`);
    }

    function buildShellMarkup(role) {
        const config = ROLE_CONFIG[role];
        const links = config.links
            .map((link) => {
                const activeClass = linkIsActive(link.href) ? " is-active" : "";
                return `<a class="pc-shell__link${activeClass}" href="${link.href}">${link.label}</a>`;
            })
            .join("");

        return `
            <div class="pc-shell" role="navigation" aria-label="Portal app shell">
                <div class="pc-shell__inner">
                    <div class="pc-shell__brand">
                        <span class="pc-shell__dot" aria-hidden="true"></span>
                        Quotey Control Plane
                    </div>
                    <div class="pc-shell__surface">Surface: ${routeLabel(window.location.pathname)}</div>
                    <div class="pc-shell__spacer" aria-hidden="true"></div>
                    <div class="pc-shell__controls">
                        <label for="pc-shell-role">Role</label>
                        <select id="pc-shell-role" name="pc-shell-role" aria-label="Select app role">
                            <option value="rep"${role === "rep" ? " selected" : ""}>Rep</option>
                            <option value="manager"${role === "manager" ? " selected" : ""}>Manager</option>
                            <option value="ops"${role === "ops" ? " selected" : ""}>Ops</option>
                        </select>
                    </div>
                    <div class="pc-shell__nav">${links}</div>
                </div>
            </div>
        `;
    }

    function bindRoleControl(root) {
        const roleSelect = root.querySelector("#pc-shell-role");
        if (!roleSelect) {
            return;
        }
        roleSelect.addEventListener("change", (event) => {
            const nextRole = normalizeRole(event.target.value);
            localStorage.setItem(ROLE_STORAGE_KEY, nextRole);
            root.innerHTML = buildShellMarkup(nextRole);
            bindRoleControl(root);
        });
    }

    function initShell() {
        const root = document.getElementById(ROOT_ID);
        if (!root) {
            return;
        }

        const role = selectRole();
        root.innerHTML = buildShellMarkup(role);
        bindRoleControl(root);
    }

    if (document.readyState === "loading") {
        document.addEventListener("DOMContentLoaded", initShell, { once: true });
    } else {
        initShell();
    }
})();
