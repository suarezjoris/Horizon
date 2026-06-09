(() => {
    const { invoke } = window.__TAURI__.core;
    const { listen } = window.__TAURI__.event;

    const cmdInput = document.getElementById('armata-cmd-input');
    const armataOutput = document.getElementById('armata-output');
    const clock = document.getElementById('armata-clock');

    // Clock
    setInterval(() => {
        clock.textContent = new Date().toLocaleTimeString('fr-FR', { hour12: false });
    }, 1000);

    function log(msg, type = 'info') {
        const line = document.createElement('div');
        line.className = `log-line log-${type}`;
        line.textContent = `[${new Date().toLocaleTimeString()}] ${msg}`;
        armataOutput.appendChild(line);
        armataOutput.scrollTop = armataOutput.scrollHeight;
        // Keep log bounded to 200 lines
        while (armataOutput.children.length > 200) {
            armataOutput.removeChild(armataOutput.firstChild);
        }
    }

    // --- Command input ---
    cmdInput.addEventListener('keydown', async (e) => {
        if (e.key !== 'Enter') return;
        const cmd = cmdInput.value.trim();
        if (!cmd) return;
        cmdInput.value = '';
        log(`> ${cmd}`, 'cmd');
        try {
            const result = await invoke('execute_armata_command', { cmd });
            log(result, 'info');
        } catch (err) {
            log(`ERROR: ${err}`, 'error');
        }
    });

    // --- Agent status events from Rust ---
    listen('armata-agent-status', (event) => {
        const { agent, status, message } = event.payload;
        updateAgentCard(agent, status, message);
        log(`[${agent.toUpperCase()}] ${message}`, status === 'error' ? 'error' : 'info');
    });

    // --- Terminal log relay from Rust ---
    listen('armata-terminal-log', (event) => {
        log(event.payload, 'info');
    });

    function updateAgentCard(agent, status, message) {
        const card = document.getElementById(`agent-${agent}`);
        if (!card) return;

        const statusEl = card.querySelector('.agent-status');
        const logEl = card.querySelector('.agent-log');
        const toggle = card.querySelector('.agent-toggle');

        if (statusEl) statusEl.textContent = message;
        if (logEl) logEl.textContent = status.toUpperCase();

        if (toggle && !toggle.classList.contains('agent-toggle--static')) {
            if (status === 'online') {
                toggle.classList.add('active');
            } else if (status === 'offline') {
                toggle.classList.remove('active');
            }
        }

        if (logEl) {
            logEl.className = 'agent-log';
            if (status === 'error') logEl.classList.add('log-error');
            else if (status === 'online') logEl.classList.add('log-online');
        }
    }

    // --- Toggle buttons ---
    document.getElementById('toggle-forge')?.addEventListener('click', async () => {
        const toggle = document.getElementById('toggle-forge');
        const isActive = toggle.classList.contains('active');
        await invoke('toggle_agent_daemon', { agent: 'forge', enabled: !isActive });
    });

    document.querySelectorAll('.agent-toggle:not(.agent-toggle--static)').forEach(toggle => {
        toggle.addEventListener('click', async () => {
            const card = toggle.closest('.agent-card');
            const agent = card.id.replace('agent-', '');
            const willEnable = !toggle.classList.contains('active');

            try {
                const result = await invoke('toggle_agent_daemon', {
                    agent,
                    enabled: willEnable,
                });
                log(result, 'info');
            } catch (err) {
                log(`Toggle error: ${err}`, 'error');
            }
        });
    });

    // --- Load initial agent states ---
    async function loadInitialState() {
        try {
            const status = await invoke('get_armata_status');
            updateAgentCard('archivist', status.archivist ? 'online' : 'offline',
                status.archivist ? 'Watching ~/Downloads' : 'Idle');
            updateAgentCard('vanguard', status.vanguard ? 'online' : 'offline',
                status.vanguard ? `Scanning every ${status.vanguard_interval}min` : 'Idle');
            updateAgentCard('antenna', status.antenna ? 'online' : 'offline',
                status.antenna ? `Bridge on :${status.antenna_port}` : 'Idle');
            updateAgentCard('forge', 'online', 'Cinema Engine Ready');
        } catch (err) {
            log(`Init error: ${err}`, 'error');
        }
    }

    window.onArmataTabActive = () => {
        log('ARMATA Command Center active.', 'info');
        loadInitialState();
    };
})();
