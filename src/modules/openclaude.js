(() => {
    const { invoke } = window.__TAURI__.core;
    const { listen } = window.__TAURI__.event;
    
    // We need the dialog plugin from Tauri
    // Make sure 'dialog' is enabled in tauri.conf.json permissions
    const openDialog = async () => {
        try {
            return await window.__TAURI__.core.invoke('plugin:dialog|open', {
                directory: true,
                multiple: false,
                title: 'Select Project Directory'
            });
        } catch (e) {
            console.error("Native dialog plugin not found, falling back to prompt", e);
            return prompt("Enter full project path:");
        }
    };

    let term;
    let fitAddon;
    let currentProjectPath = "/home/joris/Projects/Horizon";

    const pathDisplay = document.getElementById('oc-current-path');
    const changeDirBtn = document.getElementById('oc-change-dir-btn');

    async function initTerminal() {
        console.log("[Aider] Initializing terminal...");
        const container = document.getElementById('oc-xterm-container');
        if (!container || term) return;

        const LibTerminal = window.Terminal;
        if (!LibTerminal) return;

        term = new LibTerminal({
            cursorBlink: true,
            fontFamily: '"Courier New", monospace',
            fontSize: 13,
            theme: { background: '#000000', foreground: '#cccccc', cursor: '#7c3aed' }
        });

        if (window.FitAddon && window.FitAddon.FitAddon) {
            fitAddon = new window.FitAddon.FitAddon();
            term.loadAddon(fitAddon);
        }

        term.open(container);
        if (fitAddon) fitAddon.fit();

        term.onData(data => {
            invoke('send_openclaude_raw', { data }).catch(e => console.error(e));
        });

        if (window.__TAURI__ && window.__TAURI__.event) {
            window.__TAURI__.event.listen('openclaude-raw', (event) => {
                term.write(event.payload);
            });
        }

        window.addEventListener('resize', () => { if (fitAddon) fitAddon.fit(); });

        startProcess();
    }

    async function startProcess(newPath = null) {
        if (newPath) {
            currentProjectPath = newPath;
            pathDisplay.textContent = `Project: ${newPath.replace('/home/joris', '~')}`;
        }
        
        term.writeln(`\x1b[1;34mStarting Aider in ${currentProjectPath}...\x1b[0m`);
        
        try {
            await invoke('start_openclaude', { projectPath: currentProjectPath });
        } catch (e) {
            term.writeln(`\x1b[1;31mStart Error: ${e}\x1b[0m`);
        }
    }

    changeDirBtn.onclick = async () => {
        const selected = await openDialog();
        if (selected) {
            term.reset();
            await startProcess(selected);
        }
    };

    window.onCodeTabActive = () => {
        if (!term) {
            initTerminal();
        } else if (fitAddon) {
            setTimeout(() => fitAddon.fit(), 100);
        }
    };

    const panel = document.getElementById('panel-code');
    if (panel && panel.classList.contains('active')) {
        initTerminal();
    }

})();
