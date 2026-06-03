(() => {
    const { invoke } = window.__TAURI__.core;
    
    const overlay = document.getElementById('diagnostic-overlay');
    const list = document.getElementById('diagnostic-list');
    const startBtn = document.getElementById('start-app-btn');

    async function runChecks() {
        console.log("[Diagnostic] Starting system health check...");
        const timeout = setTimeout(() => {
            console.warn("[Diagnostic] Health check taking too long, showing emergency skip.");
            startBtn.disabled = false;
            startBtn.classList.add('ready');
            startBtn.textContent = "Skip Check (Emergency)";
        }, 5000);

        try {
            const results = await invoke('run_diagnostics');
            clearTimeout(timeout);
            console.log("[Diagnostic] Results received:", results);
            list.innerHTML = '';
            let allOk = true;

            results.forEach(res => {
                const item = document.createElement('div');
                item.className = `diagnostic-item ${res.status ? 'success' : 'error'}`;
                
                item.innerHTML = `
                    <div class="status-dot"></div>
                    <div class="diagnostic-info">
                        <div class="diagnostic-name">${res.name}</div>
                        <div class="diagnostic-msg">${res.message}</div>
                    </div>
                    ${!res.status && res.fixable ? `<button class="fix-btn" data-name="${res.name}">🔧 Fix</button>` : ''}
                `;
                
                list.appendChild(item);
                if (!res.status) allOk = false;
            });

            if (allOk) {
                startBtn.disabled = false;
                startBtn.classList.add('ready');
                startBtn.textContent = "Launch Horizon";
            } else {
                startBtn.textContent = "Fix Issues to Continue";
            }

            // Attach fix listeners
            document.querySelectorAll('.fix-btn').forEach(btn => {
                btn.onclick = async () => {
                    const name = btn.getAttribute('data-name');
                    btn.disabled = true;
                    btn.textContent = "Fixing...";
                    try {
                        const msg = await invoke('fix_health_issue', { name });
                        alert(msg);
                        runChecks(); // Re-run
                    } catch (e) {
                        alert("Fix failed: " + e);
                        btn.disabled = false;
                        btn.textContent = "🔧 Fix";
                    }
                };
            });

        } catch (e) {
            console.error("Diagnostic failed", e);
        }
    }

    startBtn.onclick = () => {
        overlay.style.opacity = '0';
        setTimeout(() => overlay.classList.add('hidden'), 500);
    };

    // Run automatically on load
    runChecks();

})();
