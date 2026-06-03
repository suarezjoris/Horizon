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

  unlistenToken = await listen('llm-token', e => {
    if (e.payload === "CLEAR_AND_SEARCH") {
      accumulatedText = "*🌐 Horizon is searching the web...*\n\n";
      streamingBubble.textContent = accumulatedText;
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
    await invoke('chat', { messages: trimmedMessages });
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
