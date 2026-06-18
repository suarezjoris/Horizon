let currentToolBlock = null;

window.updateIdeStatus = (status, icon = '💤') => {
    const indicator = document.getElementById('ide-status-indicator');
    if (indicator) {
        indicator.textContent = `${icon} ${status}`;
        indicator.style.color = status.includes('Executing') || status.includes('Working') ? 'var(--accent-gold)' : 'var(--text-dim)';
    }
};

window.updateIdeStatus('Idle');

window.ideSendPrompt = async () => {
    const input = document.getElementById('ide-chat-input');
    if (!input) return;
    
    const text = input.value.trim();
    if (!text) return;

    window.updateIdeStatus('Working', '🤖');
    window.ideAppendMessage(text, true);
    input.value = '';

    try {
        await window.__TAURI__.core.invoke('send_ide_prompt', { 
            prompt: text,
            mode: 'fast'
        });
    } catch (e) {
        window.ideAppendMessage("Error: " + e, false, true);
        window.updateIdeStatus('Error', '❌');
    }
};

window.ideAppendMessage = (text, isUser, isError = false, isSystem = false) => {
    const history = document.getElementById('ide-chat-history');
    if (!history) return;
    
    const div = document.createElement('div');
    div.className = 'ide-message';
    if (isUser) {
        div.classList.add('ide-message-user');
    } else if (isSystem) {
        div.classList.add('ide-message-system');
    } else if (isError) {
        div.classList.add('ide-message-error');
    } else {
        div.classList.add('ide-message-ai');
    }
    
    if (isSystem || isError) {
        div.textContent = text;
    } else {
        div.innerHTML = window.marked ? window.marked.parse(text) : `<pre style="white-space: pre-wrap; word-break: break-all;">${text}</pre>`;
    }
    
    history.appendChild(div);
    history.scrollTop = history.scrollHeight;
    currentToolBlock = null; 
};

setTimeout(() => {
    document.getElementById('ide-chat-input')?.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            window.ideSendPrompt();
        }
    });
}, 100);

async function setupIdeListeners() {
    const { listen } = window.__TAURI__.event;
    
    await listen('ide-status', (event) => {
        window.updateIdeStatus(event.payload.status, event.payload.icon || '🤖');
        if (event.payload.model) {
            document.getElementById('ide-model-indicator').textContent = '🧠 ' + event.payload.model;
        }
    });

    await listen('ide-response', (event) => {
        const payload = event.payload;
        if (currentStreamDiv) {
            currentStreamDiv.innerHTML = window.marked ? window.marked.parse(currentStreamDiv.textContent) : `<pre style="white-space: pre-wrap; word-break: break-all;">${currentStreamDiv.textContent}</pre>`;
            currentStreamDiv = null;
        } else {
            window.ideAppendMessage(payload.message, false);
        }
    });

    let currentStreamDiv = null;
    await listen('llm-token', (event) => {
        const history = document.getElementById('ide-chat-history');
        if (!history) return;

        if (!currentStreamDiv) {
            currentStreamDiv = document.createElement('div');
            currentStreamDiv.className = 'ide-message ide-message-ai';
            history.appendChild(currentStreamDiv);
        }

        // Extremely simple streaming append
        currentStreamDiv.textContent += event.payload;
        history.scrollTop = history.scrollHeight;
    });

    await listen('ide-tool-start', (event) => {
        currentStreamDiv = null; // reset stream div so next msg is new
        const history = document.getElementById('ide-chat-history');
        if (!history) return;

        const div = document.createElement('div');
        div.className = 'ide-message-tool running';
        let argsStr = "";
        try {
            argsStr = JSON.stringify(event.payload.args, null, 2);
        } catch (e) {
            argsStr = String(event.payload.args);
        }
        div.textContent = `[Running] ${event.payload.tool} ...\n${argsStr}`;
        
        history.appendChild(div);
        history.scrollTop = history.scrollHeight;
        currentToolBlock = div;
    });

    await listen('ide-tool-done', (event) => {
        if (currentToolBlock) {
            currentToolBlock.classList.remove('running');
            currentToolBlock.classList.add('success');
            currentToolBlock.textContent += `\n\n[Success]\n${event.payload.result}`;
        }
        const history = document.getElementById('ide-chat-history');
        if (history) history.scrollTop = history.scrollHeight;
    });

    await listen('ide-tool-error', (event) => {
        if (currentToolBlock) {
            currentToolBlock.classList.remove('running');
            currentToolBlock.classList.add('error');
            currentToolBlock.textContent += `\n\n[Error]\n${event.payload.error}`;
        }
        const history = document.getElementById('ide-chat-history');
        if (history) history.scrollTop = history.scrollHeight;
    });
}

(async () => {
    try {
        await setupIdeListeners();
    } catch (e) {
        console.error("Failed to setup IDE:", e);
    }
})();
