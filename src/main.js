let activeTab = 'llm';
let currentProjectPath = "/home/joris/Projects/Horizon";

function switchTab(name) {
  // Free ComfyUI models when leaving a generation tab (resource economy) — keeps
  // the model loaded while you iterate in-tab, releases RAM/VRAM once you leave.
  if ((activeTab === 'image' || activeTab === 'cinema') && activeTab !== name) {
    window.__TAURI__?.core?.invoke('free_comfyui').catch(() => {});
  }

  document.querySelectorAll('.tab').forEach(t => t.classList.toggle('active', t.dataset.tab === name));
  document.querySelectorAll('.panel').forEach(p => p.classList.toggle('active', p.id === `panel-${name}`));
  activeTab = name;

  if (name === 'image' && window.refreshGallery) {
    window.refreshGallery();
  }
  
  if (name === 'armata' && window.onArmataTabActive) {
    window.onArmataTabActive();
  }

  if (name === 'notes' && window.onNotesTabActive) {
    window.onNotesTabActive();
  }

  if (name === 'ide' && window.onIdeTabActive) {
    window.onIdeTabActive();
  }
}

// Share project path between modules
window.getCurrentProjectPath = () => currentProjectPath;
window.setCurrentProjectPath = (p) => { 
  currentProjectPath = p;
  const pathDisplay = document.getElementById('oc-current-path');
  if (pathDisplay) pathDisplay.textContent = `Project: ${p.replace('/home/joris', '~')}`;
};

// Settings overlay
let currentSettings = {};

document.getElementById('settings-btn').addEventListener('click', async () => {
  const overlay = document.getElementById('settings-overlay');
  overlay.classList.add('open');
  currentSettings = await window.__TAURI__.core.invoke('get_settings');
  document.getElementById('s-vault').value = currentSettings.vault_path;
  document.getElementById('s-llm').value = currentSettings.llm_model;
  document.getElementById('s-heavy').value = currentSettings.heavy_model || '';
  document.getElementById('s-rp').value = currentSettings.roleplay_model;
  document.getElementById('s-comfy').value = currentSettings.comfyui_path;
  document.getElementById('s-rating').value = currentSettings.image_rating || 'rating_safe';
  

  // Load MCP Store Extensions
  loadMcpStore();
});

document.getElementById('settings-overlay').addEventListener('click', e => {
  if (e.target === e.currentTarget) e.currentTarget.classList.remove('open');
});

document.getElementById('settings-save').addEventListener('click', async () => {
  const settings = {
    ...currentSettings,
    vault_path: document.getElementById('s-vault').value,
    llm_model: document.getElementById('s-llm').value,
    heavy_model: document.getElementById('s-heavy').value,
    roleplay_model: document.getElementById('s-rp').value,
    comfyui_path: document.getElementById('s-comfy').value,
    image_rating: document.getElementById('s-rating').value,


  };

  await window.__TAURI__.core.invoke('save_settings', { settings });
  currentSettings = settings;
  document.getElementById('settings-overlay').classList.remove('open');
});



// Help / Cheatsheet Modal Logic
document.getElementById('help-btn').addEventListener('click', () => {
  document.getElementById('help-overlay').classList.add('open');
});

document.getElementById('close-help-btn').addEventListener('click', () => {
  document.getElementById('help-overlay').classList.remove('open');
});

document.getElementById('help-overlay').addEventListener('click', (e) => {
  if (e.target === e.currentTarget) e.currentTarget.classList.remove('open');
});

// Persona Crafter Logic
document.getElementById('pc-save-btn').addEventListener('click', async () => {
  const name = document.getElementById('pc-name').value.trim();
  const prompt = document.getElementById('pc-prompt').value.trim();
  
  if (!name || !prompt) return alert("Please enter both a name and a system prompt.");
  
  try {
    const relPath = `characters/${name}.md`;
    await window.__TAURI__.core.invoke('write_note', { relPath, content: prompt });
    alert(`Persona '${name}' saved successfully!`);
    document.getElementById('pc-name').value = '';
    document.getElementById('pc-prompt').value = '';
    
    // Refresh the selectors in the chat bar
    if (window.refreshSelectors) await window.refreshSelectors();
  } catch (err) {
    alert("Failed to save persona: " + err);
  }
});

document.getElementById('reset-mem-btn').addEventListener('click', async () => {
  if (confirm("DANGEROUS: This will wipe all learned memories and knowledge about you. Are you sure?")) {
    try {
      const msg = await window.__TAURI__.core.invoke('reset_system');
      alert(msg);
      location.reload(); // Refresh app to trigger onboarding
    } catch (err) {
      alert("Reset failed: " + err);
    }
  }
});

document.querySelectorAll('.tab[data-tab]').forEach(tab => {
  tab.addEventListener('click', () => {
    if (!tab.classList.contains('disabled')) switchTab(tab.dataset.tab);
  });
});

window.switchTab = switchTab;

// Plugin System
(async () => {
    try {
        const { invoke } = window.__TAURI__.core;
        const plugins = await invoke('list_ui_plugins');
        
        // Find containers
        const tabContainer = document.querySelector('.sidebar') || document.querySelector('.tabs');
        const panelsContainer = document.querySelector('.main-content') || document.querySelector('.panels-container');
        
        if (!tabContainer || !panelsContainer) {
            console.warn("Could not find tab or panel containers for plugins");
            return;
        }
        
        for (const plugin of plugins) {
            // Create tab button
            const tab = document.createElement('div');
            tab.className = 'tab';
            tab.dataset.tab = `plugin-${plugin.name}`;
            tab.innerHTML = `<span>${plugin.icon || '🧩'} ${plugin.label}</span>`;
            tab.addEventListener('click', () => {
                if (!tab.classList.contains('disabled')) switchTab(tab.dataset.tab);
            });
            
            // Create panel
            const panel = document.createElement('div');
            panel.id = `panel-plugin-${plugin.name}`;
            panel.className = 'panel hidden';
            
            // Load plugin HTML
            try {
                const html = await invoke('get_plugin_html', { pluginName: plugin.name });
                panel.innerHTML = html;
            } catch (e) {
                panel.innerHTML = `<div style="padding: 20px; color: red;">Failed to load plugin UI: ${e}</div>`;
            }
            
            tabContainer.appendChild(tab);
            panelsContainer.appendChild(panel);
        }
    } catch (e) {
        console.error("Failed to load plugins:", e);
    }
})();

window.loadMcpStore = async function() {
  const container = document.getElementById('mcp-store-container');
  if (!container) return;
  
  container.innerHTML = '<div style="color:rgba(255,255,255,0.5)">Chargement du store...</div>';
  
  try {
    const servers = await window.__TAURI__.core.invoke('get_mcp_store');
    container.innerHTML = '';
    
    const countEl = document.getElementById('mcp-count');
    if (countEl) countEl.innerText = servers.length;
    
    servers.forEach(server => {
      const sDiv = document.createElement('div');
      sDiv.style.background = 'rgba(0,0,0,0.2)';
      sDiv.style.padding = '10px';
      sDiv.style.borderRadius = '6px';
      sDiv.style.border = '1px solid rgba(255,255,255,0.05)';
      
      sDiv.innerHTML = `
        <div style="display:flex; justify-content:space-between; align-items:center;">
          <strong style="color:var(--accent-gold)">${server.name}</strong>
          <button class="mcp-toggle-btn" data-id="${server.id}" style="background: ${server.installed ? 'rgba(0,255,0,0.1)' : 'rgba(255,255,255,0.1)'}; 
                         border: 1px solid ${server.installed ? 'rgba(0,255,0,0.3)' : 'rgba(255,255,255,0.2)'}; 
                         color: white; padding: 3px 8px; border-radius: 4px; cursor: pointer;">
            ${server.installed ? 'Désinstaller' : 'Installer'}
          </button>
        </div>
        <div style="color:rgba(255,255,255,0.6); margin-top: 5px;">${server.description}</div>
      `;
      
      const btn = sDiv.querySelector('.mcp-toggle-btn');
      btn.addEventListener('click', async () => {
        try {
          await window.__TAURI__.core.invoke('toggle_mcp_server', { id: server.id });
          currentSettings = await window.__TAURI__.core.invoke('get_settings');
          window.loadMcpStore(); // reload UI
        } catch (err) {
          console.error("Erreur toggle MCP:", err);
        }
      });
      
      container.appendChild(sDiv);
    });
  } catch (e) {
    container.innerHTML = `<div style="color:red">Erreur: ${e}</div>`;
  }
};
