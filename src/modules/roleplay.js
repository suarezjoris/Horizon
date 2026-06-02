(() => {
// Robust access to Tauri v2 globals
const getTauri = () => window.__TAURI__;

async function safeInvoke(cmd, args = {}) {
    const tauri = getTauri();
    if (!tauri || !tauri.core) {
        throw new Error("Tauri core not found. Are you running in a webview?");
    }
    return await tauri.core.invoke(cmd, args);
}

function getAssetUrl(path) {
    const tauri = getTauri();
    if (tauri && tauri.core && tauri.core.convertFileSrc) {
        return tauri.core.convertFileSrc(path);
    }
    if (tauri && tauri.tauri && tauri.tauri.convertFileSrc) {
        return tauri.tauri.convertFileSrc(path);
    }
    return path;
}

const safeListen = async (event, handler) => {
    const tauri = getTauri();
    return await tauri.event.listen(event, handler);
};

const rosterEl = document.getElementById('rp-roster');
const importBtn = document.getElementById('rp-import-btn');
const headerEl = document.getElementById('rp-header');
const historyEl = document.getElementById('rp-chat-history');
const inputEl = document.getElementById('rp-input');
const sendBtn = document.getElementById('rp-send-btn');

let currentCharacter = null;
let rpUnlistenToken = null;
let rpUnlistenDone = null;

async function loadRoster() {
    try {
        const chars = await safeInvoke('list_characters');
        rosterEl.innerHTML = '';
        
        for (const char of chars) {
            const card = document.createElement('div');
            card.className = `rp-char-card ${currentCharacter && currentCharacter.name === char.name ? 'active' : ''}`;
            
            const s = await safeInvoke('get_settings');
            const assetUrl = getAssetUrl(`${s.vault_path}/${char.avatar_rel_path}`);
            
            card.innerHTML = `
                <img src="" alt="">
                <div class="rp-char-info">
                    <div class="rp-char-name"></div>
                    <div class="rp-char-desc"></div>
                </div>
            `;
            card.querySelector('img').src = assetUrl;
            card.querySelector('img').alt = char.name;
            card.querySelector('.rp-char-name').textContent = char.name;
            card.querySelector('.rp-char-desc').textContent = char.description;
            
            card.onclick = () => selectCharacter(char);
            rosterEl.appendChild(card);
        }
    } catch (e) {
        console.error("Failed to load roster", e);
    }
}

function addBubble(role, text, avatarUrl = null) {
    const row = document.createElement('div');
    row.className = `bubble-row ${role}`;
    const avatar = document.createElement('div');
    avatar.className = `avatar ${role}`;
    
    if (avatarUrl && role === 'ai') {
        avatar.style.backgroundImage = `url(${avatarUrl})`;
        avatar.style.backgroundSize = 'cover';
        avatar.textContent = '';
    } else {
        avatar.textContent = role === 'ai' ? (currentCharacter ? currentCharacter.name[0] : 'H') : 'J';
    }
    
    const bubble = document.createElement('div');
    bubble.className = `bubble ${role}`;
    
    if (role === 'ai' && window.marked && window.DOMPurify) {
        bubble.innerHTML = DOMPurify.sanitize(marked.parse(text));
    } else {
        bubble.textContent = text;
    }
    
    row.append(avatar, bubble);
    historyEl.appendChild(row);
    historyEl.scrollTop = historyEl.scrollHeight;
    return bubble;
}

async function selectCharacter(char) {
    currentCharacter = char;
    headerEl.textContent = char.name;
    inputEl.disabled = false;
    sendBtn.disabled = false;
    
    Array.from(rosterEl.children).forEach(c => {
        c.classList.toggle('active', c.querySelector('.rp-char-name').textContent === char.name);
    });

    historyEl.innerHTML = '';
    
    try {
        const history = await safeInvoke('get_chat_history', { characterName: char.name });
        const s = await safeInvoke('get_settings');
        const avatarUrl = getAssetUrl(`${s.vault_path}/${char.avatar_rel_path}`);
        
        history.forEach(msg => {
            addBubble(msg.role === 'user' ? 'user' : 'ai', msg.content, avatarUrl);
        });
    } catch (e) {
        console.error("Failed to load history", e);
    }
}

importBtn.onclick = async () => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = 'image/png';
    input.onchange = async (e) => {
        const file = e.target.files[0];
        if (!file) return;
        
        const reader = new FileReader();
        reader.onload = async (event) => {
            const arrayBuffer = event.target.result;
            const bytes = Array.from(new Uint8Array(arrayBuffer));
            
            try {
                importBtn.textContent = 'Importing...';
                const char = await safeInvoke('import_character_card', { 
                    bytes, 
                    filename: file.name 
                });
                await loadRoster();
                selectCharacter(char);
            } catch (err) {
                console.error(err);
                alert("Failed to import card: " + err);
            } finally {
                importBtn.textContent = 'Import Card (.png)';
            }
        };
        reader.readAsArrayBuffer(file);
    };
    input.click();
};

async function sendRpMessage() {
    if (!currentCharacter) return;
    const text = inputEl.value.trim();
    if (!text || sendBtn.disabled) return;
    
    inputEl.value = '';
    inputEl.style.height = 'auto';
    
    addBubble('user', text);
    
    const s = await safeInvoke('get_settings');
    const avatarUrl = getAssetUrl(`${s.vault_path}/${currentCharacter.avatar_rel_path}`);
    const streamingBubble = addBubble('ai', '', avatarUrl);
    streamingBubble.classList.add('streaming');
    let accumulatedText = "";
    
    sendBtn.disabled = true;

    if (rpUnlistenToken) { await rpUnlistenToken(); rpUnlistenToken = null; }
    if (rpUnlistenDone)  { await rpUnlistenDone();  rpUnlistenDone = null; }

    rpUnlistenToken = await safeListen('llm-token', e => {
        accumulatedText += e.payload;
        streamingBubble.textContent = accumulatedText;
        historyEl.scrollTop = historyEl.scrollHeight;
    });

    rpUnlistenDone = await safeListen('llm-done', async e => {
        streamingBubble.classList.remove('streaming');
        if (window.marked && window.DOMPurify) {
            streamingBubble.innerHTML = DOMPurify.sanitize(marked.parse(e.payload));
        } else {
            streamingBubble.textContent = e.payload;
        }
        sendBtn.disabled = false;
        await rpUnlistenToken(); rpUnlistenToken = null;
        await rpUnlistenDone();  rpUnlistenDone = null;
    });

    try {
        await safeInvoke('send_roleplay_message', { 
            character: currentCharacter, 
            message: text 
        });
    } catch (err) {
        streamingBubble.textContent = `Error: ${err}`;
        streamingBubble.classList.remove('streaming');
        sendBtn.disabled = false;
    }
}

sendBtn.onclick = sendRpMessage;
inputEl.addEventListener('keydown', e => {
    if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); sendRpMessage(); }
});
inputEl.addEventListener('input', () => {
    inputEl.style.height = 'auto';
    inputEl.style.height = Math.min(inputEl.scrollHeight, 120) + 'px';
});

// Load roster on start
loadRoster();

// Enable tab
const rpTab = document.querySelector('[data-tab="roleplay"]');
if (rpTab) {
    rpTab.classList.remove('disabled');
}

})();
