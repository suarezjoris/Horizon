let activeTab = 'llm';

function switchTab(name) {
  document.querySelectorAll('.tab').forEach(t => t.classList.toggle('active', t.dataset.tab === name));
  document.querySelectorAll('.panel').forEach(p => p.classList.toggle('active', p.id === `panel-${name}`));
  activeTab = name;

  if (name === 'image' && window.refreshGallery) {
    window.refreshGallery();
  }
  
  if (name === 'code' && window.onCodeTabActive) {
    window.onCodeTabActive();
  }
}

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

document.querySelectorAll('.tab[data-tab]').forEach(tab => {
  tab.addEventListener('click', () => {
    if (!tab.classList.contains('disabled')) switchTab(tab.dataset.tab);
  });
});

window.switchTab = switchTab;
