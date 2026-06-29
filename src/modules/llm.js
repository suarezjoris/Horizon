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

listen('model-routed', (event) => {
  const { classification, model } = event.payload;
  const modelShort = model.split(':')[0].split('/').pop();
  const isComplex = classification === 'COMPLEX';
  const indicator = document.createElement('div');
  indicator.className = 'route-indicator';
  indicator.style.cssText = `
    align-self: center; text-align: center; padding: 4px 12px; margin: 4px 0;
    font-family: 'JetBrains Mono', monospace; font-size: 10px;
    letter-spacing: 0.12em; text-transform: uppercase;
    background: ${isComplex ? '#FF4D0F' : '#E8FF00'};
    color: #111110;
    border: 2px solid #111110;
  `;
  indicator.textContent = `${isComplex ? '🧠 Heavy' : '⚡ Fast'} → ${modelShort}`;
  history.appendChild(indicator);
  history.scrollTop = history.scrollHeight;
});

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

window.handleLLMDropFiles = async (files) => {
  if (files && files.length > 0) {
    filePreviewArea.style.display = 'block';
    filePreviewArea.textContent = `Extracting ${files.length} file(s)...`;
    
    try {
      for (const file of files) {
        const content = await invoke('read_file_content', { path: file });
        if (content.type === "Image") {
           attachedFilesText += `\n\n--- Image Attached ---\n[Base64 Image Data - Hidden]\n---\n`;
        }
        
        let textContent = content.data || content; // support old and new formats
        attachedFilesText += `\n\n--- Content from ${file} ---\n${textContent}\n---\n`;
        attachedFileNames.push(file.split('/').pop().split('\\').pop());
      }
      filePreviewArea.textContent = `Attached: ${attachedFileNames.join(', ')}`;
    } catch (err) {
      filePreviewArea.textContent = `Error reading file: ${err}`;
    }
  }
};

attachBtn.addEventListener('click', async () => {
  try {
    const file = await window.__TAURI__.dialog.open({
      multiple: false,
      title: 'Select a file to extract'
    });
    if (file) {
      window.handleLLMDropFiles([file]);
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

  if (curiosityAnswered && userText) {
    invoke('curiosity_mark_answered', { question: curiosityAnswered }).catch(() => {});
    curiosityAnswered = null;
  }
  resetCuriosityIdle();

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

  if (messages.length > 20) {
    const toConsolidate = messages.slice(0, messages.length - 6);
    messages = messages.slice(messages.length - 6);
    addBubble('system', '*(Historique long détecté : consolidation automatique en tâche de fond pour économiser la mémoire...)*');
    invoke('auto_consolidate_chat', { history: toConsolidate })
      .then(() => console.log('Auto-consolidation success'))
      .catch(e => console.error('Auto-consolidation failed', e));
  }
  
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

    if (cmd === 'save' || cmd === 'remember') {
      if (!query) { addBubble('ai', 'Usage: /save <note_name>  — saves the last AI response to that vault note.'); return; }
      // Find last AI bubble content to save
      const bubbles = document.querySelectorAll('.bubble.ai');
      const lastAI = bubbles[bubbles.length - 1];
      const content = lastAI ? lastAI.textContent.trim() : '';
      if (!content) { addBubble('ai', 'Nothing to save — no AI response found.'); return; }
      const bubble = addBubble('ai', `Saving to ${query}.md…`);
      try {
        const msg = await invoke('save_to_note', { noteHint: query, content });
        bubble.textContent = msg;
      } catch (err) {
        bubble.textContent = `Save failed: ${err}`;
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
    }

    if (cmd === 'xlsx' || cmd === 'excel') {
      addBubble('user', `Generating Excel file: ${query}`);
      messages.push({ role: 'user', content: `GENERATE_XLSX for: ${query}.` });
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
      addBubble('user', `Création d'une présentation : ${query}`);
      messages.push({ role: 'user', content: `/pptx ${query}` });
      
      const aiBubble = addBubble('ai', '🎨 Analyse de votre demande et recherche de designs premium en cours...');
      aiBubble.classList.add('streaming');
      
      (async () => {
          try {
              const keywords = await invoke('analyze_pptx_request', { prompt: query });
              aiBubble.textContent = `Thème identifié : ${keywords}. Recherche de templates...`;

              const templates = await invoke('scrape_pptx_templates', { query: keywords });
              
              aiBubble.classList.remove('streaming');
              aiBubble.innerHTML = `Voici quelques styles pour votre document, lequel préférez-vous ?<br><div class="template-carousel"></div>`;
              const carousel = aiBubble.querySelector('.template-carousel');
              
              templates.forEach(tpl => {
                  const card = document.createElement('div');
                  card.className = 'template-card';
                  const img = document.createElement('img');
                  img.src = tpl.thumbnail_url;
                  img.alt = tpl.title;
                  const info = document.createElement('div');
                  info.className = 'template-info';
                  const titleDiv = document.createElement('div');
                  titleDiv.className = 'template-title';
                  titleDiv.textContent = tpl.title;
                  const sourceDiv = document.createElement('div');
                  sourceDiv.className = 'template-source';
                  sourceDiv.textContent = tpl.source;
                  info.append(titleDiv, sourceDiv);
                  card.append(img, info);
                  card.onclick = async () => {
                      aiBubble.replaceChildren();
                      const genStrong = document.createElement('strong');
                      genStrong.textContent = tpl.title;
                      aiBubble.append('Génération de la structure et injection XML native dans le style ', genStrong, '... Cela prend environ une minute.');
                      aiBubble.classList.add('streaming');

                      try {
                          const finalPath = await invoke('execute_pptx_generation', { prompt: query, templateUrl: tpl.download_url });
                          aiBubble.classList.remove('streaming');
                          const filename = finalPath.split('/').pop().split('\\').pop();
                          aiBubble.replaceChildren();
                          const magicStrong = document.createElement('strong');
                          magicStrong.textContent = 'Magie opérée !';
                          const fileCode = document.createElement('code');
                          fileCode.textContent = filename;
                          const openBtn = document.createElement('button');
                          openBtn.className = 'office-gen-btn';
                          openBtn.textContent = '📂 Ouvrir le document';
                          openBtn.onclick = () => window.__TAURI__.core.invoke('open_docs_folder');
                          aiBubble.append('✨ ', magicStrong, document.createElement('br'), 'Votre présentation ', fileCode, ' a été générée en un éclair.', document.createElement('br'), document.createElement('br'), openBtn);
                      } catch (e) {
                          aiBubble.classList.remove('streaming');
                          aiBubble.textContent = `❌ Erreur lors de la génération : ${e}`;
                      }
                  };
                  carousel.appendChild(card);
              });
          } catch (e) {
              aiBubble.classList.remove('streaming');
              aiBubble.textContent = `Erreur : ${e}`;
          }
      })();
      
      input.value = '';
      input.style.height = 'auto';
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

    if (cmd === 'topics') {
      const bubble = addBubble('ai', 'Analyzing vault topics…');
      try {
        const status = await invoke('vault_topic_status');
        const hubLines = status.hubs.map(h => `- [[${h.name}]] — ${h.count} note(s)`).join('\n');
        const uncatList = status.uncategorized.length
          ? status.uncategorized.slice(0, 10).join(', ') + (status.uncategorized.length > 10 ? ` (+${status.uncategorized.length - 10} more)` : '')
          : 'none';
        bubble.innerHTML = DOMPurify.sanitize(marked.parse(
          `## Vault Topic Health\n\n**Active hubs** (${status.hubs.length}):\n${hubLines}\n\n` +
          `**Uncategorized notes** (${status.uncategorized_count}): ${uncatList}`
        ));
      } catch (err) {
        bubble.textContent = `Failed: ${err}`;
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

  let unlistenThinking, unlistenToolStart, unlistenToolDone, unlistenToolError;
  let activeToolArgs = null;
  const cleanupAgentListeners = async () => {
    if (unlistenThinking)  { await unlistenThinking();  unlistenThinking = null; }
    if (unlistenToolStart) { await unlistenToolStart(); unlistenToolStart = null; }
    if (unlistenToolDone)  { await unlistenToolDone();  unlistenToolDone = null; }
    if (unlistenToolError) { await unlistenToolError(); unlistenToolError = null; }
  };

  unlistenThinking = await listen('agent-thinking', e => {
    if (e.payload) {
      streamingBubble.textContent = '...';
    }
  });

  unlistenToolStart = await listen('agent-tool-start', e => {
    const { tool, args } = e.payload;
    activeToolArgs = args;
    const argStr = Object.entries(args || {}).map(([k,v]) => `${k}=${JSON.stringify(v)}`).join(', ');
    streamingBubble.textContent = `[${tool}(${argStr})]`;
  });

  unlistenToolDone = await listen('agent-tool-done', e => {
    const { tool, result, ms } = e.payload;
    if (tool === 'edit_file' && activeToolArgs) {
        const { search, replace } = activeToolArgs;
        renderDiffPreview(streamingBubble, activeToolArgs.path, search, replace);
    }
    const preview = (typeof result === 'string' ? result : JSON.stringify(result)).substring(0, 80).replace(/\n/g, ' ');
    streamingBubble.textContent = `[${tool} → ${preview || '(done)'}]`;
    activeToolArgs = null;
  });

  unlistenToolError = await listen('agent-tool-error', e => {
    const { tool, error } = e.payload;
    streamingBubble.textContent = `[${tool} error: ${error}]`;
    activeToolArgs = null;
  });

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

    if (e.payload.startsWith("OFFICE_GEN_SUCCESS:")) {
        const path = e.payload.split("OFFICE_GEN_SUCCESS:")[1];
        const filename = path.split('/').pop().split('\\').pop();
        
        accumulatedText += `\n\n📄 **Document prêt :** \`${filename}\``;
        streamingBubble.innerHTML = DOMPurify.sanitize(marked.parse(accumulatedText));
        
        const btn = document.createElement('button');
        btn.textContent = "📂 Ouvrir le dossier documents";
        btn.className = "office-gen-btn";
        btn.style = "margin-top: 15px; display: block; background: var(--signal,#E8FF00); color: #111110; border: 2px solid #111110; padding: 10px 16px; cursor: pointer; font-family: 'JetBrains Mono', monospace; font-size: 11px; font-weight: 800; letter-spacing: 0.1em; text-transform: uppercase; clip-path: polygon(0 0,100% 0,100% calc(100% - 9px),calc(100% - 9px) 100%,0 100%); transition: transform 0.2s, box-shadow 0.2s;";
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
        enhanceCodeBlocks(streamingBubble);
    } else {
        streamingBubble.textContent = fullMsg;
    }
    messages.push({ role: 'assistant', content: fullMsg });
    sendBtn.disabled = false;
    await unlistenToken(); unlistenToken = null;
    await unlistenDone();  unlistenDone = null;
    await cleanupAgentListeners();

    const match = fullMsg.match(/GENERATE_IMAGE:(.+)/);
    if (match) {
      const prompt = match[1].trim();
      if (window.switchTab) {
        window.switchTab('image');
        const promptEl = document.getElementById('image-prompt');
        if (promptEl) {
          promptEl.value = prompt;
          // Don't auto-trigger generation — user must confirm by clicking Generate.
        }
      }
    }
  });

  try {
    const trimmedMessages = messages.slice(-15);
    const model = modelSelect.value || null;
    const persona = personaSelect.value || null;
    await invoke('chat', { messages: trimmedMessages, model, persona });
  } catch (err) {
    streamingBubble.textContent = `Error: ${err}`;
    streamingBubble.classList.remove('streaming');
    sendBtn.disabled = false;
    await cleanupAgentListeners();
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

const exportPdfBtn = document.getElementById('export-pdf-btn');
if (exportPdfBtn) {
  exportPdfBtn.addEventListener('click', async () => {
    const bubbles = document.querySelectorAll('.bubble');
    if (bubbles.length === 0) {
      alert("Chat is empty.");
      return;
    }
    
    const visualMessages = Array.from(bubbles).map(b => {
      let role = 'system';
      if (b.classList.contains('user')) role = 'user';
      else if (b.classList.contains('ai')) role = 'assistant';
      return { role, content: b.innerText || b.textContent };
    });

    const originalText = exportPdfBtn.textContent;
    exportPdfBtn.textContent = '⏳ Exporting...';
    exportPdfBtn.disabled = true;
    try {
      const path = await invoke('export_chat_as_pdf', { messages: visualMessages });
      addBubble('system', `Chat exported as PDF: ${path}`);
    } catch (e) {
      alert(`Export failed: ${e}`);
    } finally {
      exportPdfBtn.textContent = originalText;
      exportPdfBtn.disabled = false;
    }
  });
}

let pendingPaxBannerQuestion = null;
let curiosityAnswered = null; // question awaiting the user's reply, to mark answered
let idleTimer = null;
const IDLE_MS = 150000; // 2.5 min
let pendingTopic = null;
let curiosityCycles = 0;

function resetCuriosityIdle() {
    if (idleTimer) clearTimeout(idleTimer);
    idleTimer = setTimeout(fireCuriosity, IDLE_MS);
}

async function fireWorldLane() {
    if (document.getElementById('topic-banner').style.display === 'block') return false;
    try {
        const topic = await invoke('curiosity_propose_topic');
        if (!topic) return false;
        pendingTopic = topic;
        document.getElementById('topic-banner-text').textContent =
            `Le sujet « ${topic} » t'intéresse mais je n'ai rien dessus. Je l'approfondis ?`;
        document.getElementById('topic-banner').style.display = 'block';
        return true;
    } catch (e) { console.error('topic propose failed', e); return false; }
}

async function fireCuriosity() {
    if (document.getElementById('pax-banner').style.display === 'block') return;
    curiosityCycles++;
    if (curiosityCycles % 3 === 0 && await fireWorldLane()) return;
    try {
        const q = await invoke('curiosity_next_question');
        if (!q) { resetCuriosityIdle(); return; }
        pendingPaxBannerQuestion = q;
        document.getElementById('pax-banner-question').textContent = q;
        document.getElementById('pax-banner').style.display = 'block';
    } catch (e) {
        console.error('curiosity failed', e);
    }
}

document.getElementById('pax-banner-open')?.addEventListener('click', () => {
    document.getElementById('pax-banner').style.display = 'none';
    if (!pendingPaxBannerQuestion) return;
    switchTab('llm');
    addBubble('ai', pendingPaxBannerQuestion);
    messages.push({ role: 'assistant', content: pendingPaxBannerQuestion });
    curiosityAnswered = pendingPaxBannerQuestion;
    pendingPaxBannerQuestion = null;
    history.scrollTop = history.scrollHeight;
    input.focus();
});

document.getElementById('pax-banner-dismiss')?.addEventListener('click', () => {
    document.getElementById('pax-banner').style.display = 'none';
    pendingPaxBannerQuestion = null;
    resetCuriosityIdle();
});

document.getElementById('topic-banner-confirm')?.addEventListener('click', async () => {
    document.getElementById('topic-banner').style.display = 'none';
    if (!pendingTopic) return;
    const t = pendingTopic; pendingTopic = null;
    addBubble('system', `*Recherche en cours sur « ${t} »...*`);
    try {
        const msg = await invoke('curiosity_fill_topic', { topic: t });
        addBubble('system', `*${msg}*`);
    } catch (e) { addBubble('system', `*Échec: ${e}*`); }
    resetCuriosityIdle();
});

document.getElementById('topic-banner-dismiss')?.addEventListener('click', () => {
    document.getElementById('topic-banner').style.display = 'none';
    if (pendingTopic) { invoke('curiosity_dismiss_topic', { topic: pendingTopic }).catch(() => {}); }
    pendingTopic = null;
    resetCuriosityIdle();
});

['keydown', 'click'].forEach(evt => {
    history?.addEventListener(evt, resetCuriosityIdle);
    input?.addEventListener(evt, resetCuriosityIdle);
});
resetCuriosityIdle();

function isRunnable(lang) {
    return ['python', 'python3', 'bash', 'sh', 'javascript', 'js', 'node', 'rust'].includes(lang.toLowerCase());
}

function detectLanguage(codeBlock) {
    const classList = Array.from(codeBlock.classList);
    const langClass = classList.find(c => c.startsWith('language-'));
    return langClass ? langClass.replace('language-', '') : '';
}

function enhanceCodeBlocks(bubbleEl) {
    bubbleEl.querySelectorAll('pre > code').forEach(block => {
        const lang = detectLanguage(block);
        if (!lang) return;
        
        const pre = block.parentElement;
        if (pre.previousElementSibling && pre.previousElementSibling.classList.contains('code-toolbar')) return;

        const toolbar = document.createElement('div');
        toolbar.className = 'code-toolbar';
        
        const langBadge = document.createElement('span');
        langBadge.className = 'lang-badge';
        langBadge.textContent = lang;
        toolbar.appendChild(langBadge);

        const actions = document.createElement('div');
        actions.className = 'code-actions';

        if (isRunnable(lang)) {
            const runBtn = document.createElement('button');
            runBtn.innerHTML = '▶ Run';
            runBtn.className = 'run-btn';
            runBtn.onclick = async () => {
                const code = block.textContent;
                let outputPanel = pre.nextElementSibling;
                if (!outputPanel || !outputPanel.classList.contains('output-panel')) {
                    outputPanel = document.createElement('pre');
                    outputPanel.className = 'output-panel';
                    pre.parentElement.insertBefore(outputPanel, pre.nextSibling);
                }
                
                outputPanel.style.display = 'block';
                outputPanel.textContent = 'Running...';
                
                try {
                    const result = await invoke('execute_code_preview', { code, language: lang });
                    outputPanel.textContent = result.stdout;
                } catch (err) {
                    outputPanel.textContent = `Error: ${err}`;
                }
            };
            actions.appendChild(runBtn);
        }

        const copyBtn = document.createElement('button');
        copyBtn.innerHTML = '📋 Copy';
        copyBtn.className = 'copy-btn';
        copyBtn.onclick = () => {
            navigator.clipboard.writeText(block.textContent);
            copyBtn.innerHTML = '✅ Copied';
            setTimeout(() => copyBtn.innerHTML = '📋 Copy', 2000);
        };
        actions.appendChild(copyBtn);

        toolbar.appendChild(actions);
        pre.parentElement.insertBefore(toolbar, pre);
    });
}

function renderDiffPreview(streamingBubble, path, search, replace) {
    const row = document.createElement('div');
    row.className = 'bubble-row ai';
    const avatar = document.createElement('div');
    avatar.className = 'avatar ai';
    avatar.textContent = 'H';
    
    const diffBubble = document.createElement('div');
    diffBubble.className = 'bubble ai diff-preview';
    const diffHeader = document.createElement('strong');
    diffHeader.append('File Edited: ');
    const pathCode = document.createElement('code');
    pathCode.textContent = path;
    diffHeader.appendChild(pathCode);
    diffBubble.appendChild(diffHeader);
    
    const diffPanel = document.createElement('div');
    diffPanel.className = 'diff-panel';
    
    const searchLines = (search || '').split('\n');
    const replaceLines = (replace || '').split('\n');

    searchLines.forEach(line => {
        const div = document.createElement('div');
        div.className = 'diff-line diff-removed';
        div.textContent = '- ' + line;
        diffPanel.appendChild(div);
    });
    
    replaceLines.forEach(line => {
        const div = document.createElement('div');
        div.className = 'diff-line diff-added';
        div.textContent = '+ ' + line;
        diffPanel.appendChild(div);
    });
    
    diffBubble.appendChild(diffPanel);
    row.append(avatar, diffBubble);

    const streamRow = streamingBubble.parentElement;
    streamRow.parentElement.insertBefore(row, streamRow);
}
