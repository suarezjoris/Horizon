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
  document.getElementById('s-rp').value = currentSettings.roleplay_model;
  document.getElementById('s-comfy').value = currentSettings.comfyui_path;
  document.getElementById('s-rating').value = currentSettings.image_rating || 'rating_safe';
});

document.getElementById('settings-overlay').addEventListener('click', e => {
  if (e.target === e.currentTarget) e.currentTarget.classList.remove('open');
});

document.getElementById('settings-save').addEventListener('click', async () => {
  const settings = {
    ...currentSettings,
    vault_path: document.getElementById('s-vault').value,
    llm_model: document.getElementById('s-llm').value,
    roleplay_model: document.getElementById('s-rp').value,
    comfyui_path: document.getElementById('s-comfy').value,
    image_rating: document.getElementById('s-rating').value
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
