const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const history = document.getElementById('chat-history');
const input = document.getElementById('chat-input');
const sendBtn = document.getElementById('send-btn');
const modelSelect = document.getElementById('model-select');
const personaSelect = document.getElementById('persona-select');

let messages = [];
let streamingBubble = null;
let unlistenToken = null;
let unlistenDone = null;

async function refreshSelectors() {
    try {
        const models = await invoke('list_ollama_models');
        modelSelect.innerHTML = '<option value="">Default Model</option>';
        models.forEach(m => {
            const opt = document.createElement('option');
            opt.value = m;
            opt.textContent = m;
            modelSelect.appendChild(opt);
        });

        const personas = await invoke('list_personas');
        personaSelect.innerHTML = '<option value="">🎭 Horizon</option>';
        personas.forEach(p => {
            const opt = document.createElement('option');
            opt.value = p;
            opt.textContent = `🎭 ${p}`;
            personaSelect.appendChild(opt);
        });
    } catch (e) {
        console.error("Failed to refresh selectors", e);
    }
}
window.refreshSelectors = refreshSelectors;
refreshSelectors();


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

const micBtn = document.getElementById('mic-btn');
let mediaRecorder = null;
let audioChunks = [];

micBtn.addEventListener('click', async () => {
  if (mediaRecorder && mediaRecorder.state === 'recording') {
    mediaRecorder.stop();
    micBtn.classList.remove('recording');
    return;
  }

  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    mediaRecorder = new MediaRecorder(stream);
    audioChunks = [];

    mediaRecorder.ondataavailable = (event) => {
      audioChunks.push(event.data);
    };

    mediaRecorder.onstop = async () => {
      const audioBlob = new Blob(audioChunks, { type: 'audio/webm' });
      const reader = new FileReader();
      reader.readAsDataURL(audioBlob);
      reader.onloadend = async () => {
        const base64data = reader.result.split(',')[1];
        micBtn.style.opacity = '0.5';
        input.placeholder = "Transcribing audio...";
        
        try {
          const tempPath = await invoke('save_audio_temp', { base64Data: base64data });
          const text = await invoke('transcribe_audio', { audioPath: tempPath });
          input.value += (input.value ? " " : "") + text;
          input.placeholder = "Ask anything or drop a file here...";
          input.dispatchEvent(new Event('input')); // trigger auto-resize
        } catch (err) {
          alert("Transcription error: " + err);
        } finally {
          micBtn.style.opacity = '1';
        }
      };
      
      // Stop all tracks to release microphone
      stream.getTracks().forEach(track => track.stop());
    };

    mediaRecorder.start();
    micBtn.classList.add('recording');
  } catch (err) {
    alert("Microphone access denied or not available: " + err);
  }
});

const attachBtn = document.getElementById('attach-btn');
const filePreviewArea = document.getElementById('file-preview-area');
let attachedFilesText = "";
let attachedFileNames = [];

attachBtn.addEventListener('click', async () => {
  try {
    const file = await window.__TAURI__.dialog.open({
      multiple: false,
      title: 'Select a file to extract'
    });
    if (file) {
      filePreviewArea.style.display = 'block';
      filePreviewArea.textContent = `Extracting ${file}...`;
      
      const content = await invoke('read_file_content', { path: file });
      attachedFilesText += `\n\n--- Content from ${file} ---\n${content}\n---\n`;
      attachedFileNames.push(file.split('/').pop().split('\\').pop());
      
      filePreviewArea.textContent = `Attached: ${attachedFileNames.join(', ')}`;
    }
  } catch (err) {
    filePreviewArea.style.display = 'block';
    filePreviewArea.textContent = `Error reading file: ${err}`;
  }
});

async function send() {
  let userText = input.value.trim();
  let llmText = userText;
  let displayText = userText;

  if (!userText && !attachedFilesText) return;
  if (sendBtn.disabled) return;
  
  if (attachedFilesText) {
    llmText += "\n" + attachedFilesText;
    displayText += "\n\n📎 Attached: " + attachedFileNames.join(', ');
    attachedFilesText = "";
    attachedFileNames = [];
    filePreviewArea.style.display = 'none';
    filePreviewArea.textContent = '';
  }

  input.value = '';
  input.style.height = 'auto';
  
  // Handle Commands
  if (llmText.startsWith('/')) {
    const [cmd, ...args] = llmText.slice(1).split(' ');
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

    if (cmd === 'consolidate') {
      const bubble = addBubble('ai', 'Consolidating neurons... refactoring the Second Brain.');
      try {
        const msg = await invoke('consolidate_vault');
        bubble.textContent = msg;
      } catch (err) {
        bubble.textContent = `Consolidation failed: ${err}`;
      }
      return;
    }

    if (cmd === 'docx' || cmd === 'word') {
      addBubble('user', `Generating Word document: ${query}`);
      messages.push({ role: 'user', content: `GENERATE_DOCX for: ${query}. Use search if needed for factual accuracy.` });
      // Let the normal send logic handle it from here
    }

    if (cmd === 'xlsx' || cmd === 'excel') {
      addBubble('user', `Generating Excel file: ${query}`);
      messages.push({ role: 'user', content: `GENERATE_XLSX for: ${query}.` });
      // Let the normal send logic handle it from here
    }

    if (cmd === 'wiki' || cmd === 'learn_wiki') {
      const bubble = addBubble('ai', 'Scanning Wikipedia for vault seeds… reading ZIM file, this may take a few minutes.');
      listen('wiki-ingest-status', e => {
        if (e.payload?.message) bubble.textContent = e.payload.message;
      });
      try {
        const msg = await invoke('ingest_wikipedia');
        bubble.textContent = msg;
      } catch (err) {
        bubble.textContent = `Ingestion failed: ${err}`;
      }
      return;
    }

    if (cmd === 'ppt' || cmd === 'pptx' || cmd === 'powerpoint') {
      addBubble('user', `Generating PowerPoint: ${query}`);
      messages.push({ role: 'user', content: `GENERATE_PPTX for: ${query}. Use search for depth and accuracy.` });
      // Let the normal send logic handle it from here
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
  addBubble('user', displayText);
  messages.push({ role: 'user', content: llmText });

  streamingBubble = addBubble('ai', '');
  streamingBubble.classList.add('streaming');
  let accumulatedText = "";

  if (unlistenToken) { await unlistenToken(); unlistenToken = null; }
  if (unlistenDone)  { await unlistenDone();  unlistenDone = null; }

  unlistenToken = await listen('llm-token', async e => {
    if (e.payload === "CLEAR_AND_SEARCH") {
      accumulatedText = "*🌐 Horizon is searching the web...*\n\n";
      streamingBubble.textContent = accumulatedText;
      return;
    }
    
    // Hide raw trigger tags from UI
    if (e.payload.startsWith("GENERATE_DOCX:") || e.payload.startsWith("GENERATE_XLSX:") || e.payload.startsWith("SEARCH_WEB:")) {
        return;
    }

    // Detect document generation success in stream and provide a button
    if (e.payload.startsWith("OFFICE_GEN_SUCCESS:")) {
        const path = e.payload.split("OFFICE_GEN_SUCCESS:")[1];
        const filename = path.split('/').pop().split('\\').pop();
        
        accumulatedText += `\n\n📄 **Document prêt :** \`${filename}\``;
        streamingBubble.innerHTML = DOMPurify.sanitize(marked.parse(accumulatedText));
        
        const btn = document.createElement('button');
        btn.innerHTML = "📂 Ouvrir le dossier documents";
        btn.className = "office-gen-btn";
        btn.style = "margin-top: 15px; display: block; background: var(--accent-gold-strong); color: #000; border: none; padding: 10px 16px; border-radius: 12px; cursor: pointer; font-size: 12px; font-weight: 800; box-shadow: 0 4px 15px rgba(212,175,55,0.3); transition: all 0.2s;";
        btn.onclick = () => invoke('open_docs_folder');
        streamingBubble.appendChild(btn);
        return;
    }

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
    const model = modelSelect.value || null;
    const persona = personaSelect.value || null;
    await invoke('chat', { messages: trimmedMessages, model, persona });
  } catch (err) {
    streamingBubble.textContent = `Error: ${err}`;
    streamingBubble.classList.remove('streaming');
    sendBtn.disabled = false;
  }
}

// Drag and Drop support
listen('tauri://file-drop', async event => {
  const activeTab = document.querySelector('.tab.active');
  if (activeTab && activeTab.dataset.tab !== 'llm') return;
  
  const files = event.payload;
  if (files && files.length > 0) {
    filePreviewArea.style.display = 'block';
    filePreviewArea.textContent = `Extracting ${files.length} file(s)...`;
    
    try {
      for (const file of files) {
        const content = await invoke('read_file_content', { path: file });
        attachedFilesText += `\n\n--- Content from ${file} ---\n${content}\n---\n`;
        attachedFileNames.push(file.split('/').pop().split('\\').pop());
      }
      filePreviewArea.textContent = `Attached: ${attachedFileNames.join(', ')}`;
    } catch (err) {
      filePreviewArea.textContent = `Error reading file: ${err}`;
    }
  }
});

sendBtn.addEventListener('click', send);
input.addEventListener('keydown', e => {
  if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); send(); }
});
input.addEventListener('input', () => {
  input.style.height = 'auto';
  input.style.height = Math.min(input.scrollHeight, 120) + 'px';
});
