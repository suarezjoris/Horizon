const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const history = document.getElementById('chat-history');
const input = document.getElementById('chat-input');
const sendBtn = document.getElementById('send-btn');

let messages = [];
let streamingBubble = null;
let unlistenToken = null;
let unlistenDone = null;

function addBubble(role, text) {
  const row = document.createElement('div');
  row.className = `bubble-row ${role}`;
  const avatar = document.createElement('div');
  avatar.className = `avatar ${role}`;
  avatar.textContent = role === 'ai' ? 'H' : 'J';
  const bubble = document.createElement('div');
  bubble.className = `bubble ${role}`;
  
  if (role === 'ai' && window.marked && window.DOMPurify) {
    bubble.innerHTML = DOMPurify.sanitize(marked.parse(text));
  } else {
    bubble.textContent = text;
  }
  
  row.append(avatar, bubble);
  history.appendChild(row);
  history.scrollTop = history.scrollHeight;
  return bubble;
}

async function send() {
  const text = input.value.trim();
  if (!text || sendBtn.disabled) return;
  input.value = '';
  input.style.height = 'auto';
  
  // Handle Commands
  if (text.startsWith('/')) {
    const [cmd, ...args] = text.slice(1).split(' ');
    const query = args.join(' ');

    if (cmd === 'clear') {
      messages = [];
      history.innerHTML = '';
      addBubble('ai', 'Chat history cleared.');
      return;
    }

    if (cmd === 'reindex') {
      const bubble = addBubble('ai', 'Reindexing vault... this may take a minute.');
      try {
        const count = await invoke('reindex');
        bubble.textContent = `Reindex complete! ${count} chunks indexed.`;
      } catch (err) {
        bubble.textContent = `Reindex failed: ${err}`;
      }
      return;
    }

    if (cmd === 'search') {
      if (!query) { addBubble('ai', 'Usage: /search <query>'); return; }
      addBubble('user', `Searching for: ${query}`);
      try {
        const results = await invoke('search_vault', { query });
        if (results.length === 0) {
          addBubble('ai', 'No results found.');
        } else {
          addBubble('ai', 'Top matches:\n\n' + results.join('\n\n---\n\n'));
        }
      } catch (err) {
        addBubble('ai', `Search failed: ${err}`);
      }
      return;
    }

    if (cmd === 'memory') {
      addBubble('ai', 'Reading core memory...');
      try {
        const vault_path = (await invoke('get_settings')).vault_path;
        const user = await invoke('read_note', { path: 'memory/user.md' });
        const code = await invoke('read_note', { path: 'memory/code.md' });
        const skills = await invoke('read_note', { path: 'memory/skills.md' });
        addBubble('ai', `USER:\n${user}\n\nCODE:\n${code}\n\nSKILLS:\n${skills}`);
      } catch (err) {
        addBubble('ai', `Failed to read memory: ${err}`);
      }
      return;
    }
  }

  sendBtn.disabled = true;
  addBubble('user', text);
  messages.push({ role: 'user', content: text });

  streamingBubble = addBubble('ai', '');
  streamingBubble.classList.add('streaming');
  let accumulatedText = "";

  if (unlistenToken) { await unlistenToken(); unlistenToken = null; }
  if (unlistenDone)  { await unlistenDone();  unlistenDone = null; }

  unlistenToken = await listen('llm-token', e => {
    accumulatedText += e.payload;
    streamingBubble.textContent = accumulatedText;
    history.scrollTop = history.scrollHeight;
  });

  unlistenDone = await listen('llm-done', async e => {
    streamingBubble.classList.remove('streaming');
    const fullMsg = e.payload;
    if (window.marked && window.DOMPurify) {
        streamingBubble.innerHTML = DOMPurify.sanitize(marked.parse(fullMsg));
    } else {
        streamingBubble.textContent = fullMsg;
    }
    messages.push({ role: 'assistant', content: fullMsg });
    sendBtn.disabled = false;
    await unlistenToken(); unlistenToken = null;
    await unlistenDone();  unlistenDone = null;

    // Cross-module trigger: switch to Image tab if LLM requests image generation
    const match = fullMsg.match(/GENERATE_IMAGE:(.+)/);
    if (match) {
      const prompt = match[1].trim();
      if (window.switchTab) {
        window.switchTab('image');
        const promptEl = document.getElementById('image-prompt');
        if (promptEl) {
          promptEl.value = prompt;
          // VULN-007 Fix: We no longer auto-trigger window.triggerImageGeneration() 
          // The user must explicitly click the 'Generate' button to confirm.
        }
      }
    }
  });

  try {
    // SCALE-001 Fix: Limit context window to last 15 messages
    const trimmedMessages = messages.slice(-15);
    await invoke('chat', { messages: trimmedMessages });
  } catch (err) {
    streamingBubble.textContent = `Error: ${err}`;
    streamingBubble.classList.remove('streaming');
    sendBtn.disabled = false;
  }
}

sendBtn.addEventListener('click', send);
input.addEventListener('keydown', e => {
  if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); send(); }
});
input.addEventListener('input', () => {
  input.style.height = 'auto';
  input.style.height = Math.min(input.scrollHeight, 120) + 'px';
});
