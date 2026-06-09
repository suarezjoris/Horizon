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

// ── Custom context menu for gallery images ──────────────────────────────────
let ctxMenu = null;

function buildContextMenu() {
    const menu = document.createElement('div');
    menu.id = 'img-ctx-menu';
    menu.style.cssText = 'position:fixed;display:none;z-index:9999;background:#1a1a2e;border:1px solid #444;border-radius:6px;padding:4px 0;min-width:170px;box-shadow:0 4px 16px rgba(0,0,0,.6)';
    const useBaseItem = document.createElement('div');
    useBaseItem.className = 'ctx-item';
    useBaseItem.id = 'ctx-use-base';
    useBaseItem.textContent = '✏️ Use as base (img2img)';
    const copyImgItem = document.createElement('div');
    copyImgItem.className = 'ctx-item';
    copyImgItem.id = 'ctx-copy-img';
    copyImgItem.textContent = '🖼️ Copy image';
    const saveItem = document.createElement('div');
    saveItem.className = 'ctx-item';
    saveItem.id = 'ctx-save';
    saveItem.textContent = '💾 Save to Downloads';
    const copyItem = document.createElement('div');
    copyItem.className = 'ctx-item';
    copyItem.id = 'ctx-copy-path';
    copyItem.textContent = '📋 Copy file path';
    menu.appendChild(useBaseItem);
    menu.appendChild(copyImgItem);
    menu.appendChild(saveItem);
    menu.appendChild(copyItem);
    const style = document.createElement('style');
    style.textContent = '.ctx-item{padding:8px 16px;cursor:pointer;font-size:13px;color:#ccc}.ctx-item:hover{background:#2a2a4e;color:#fff}';
    document.head.appendChild(style);
    document.body.appendChild(menu);
    document.addEventListener('click', () => { menu.style.display = 'none'; });
    document.addEventListener('contextmenu', () => { menu.style.display = 'none'; });
    return menu;
}

function showContextMenu(e, realPath) {
    e.preventDefault();
    e.stopPropagation();
    if (!ctxMenu) ctxMenu = buildContextMenu();

    ctxMenu.querySelector('#ctx-use-base').onclick = () => {
        ctxMenu.style.display = 'none';
        setI2ISource(realPath);
        i2iEnabled.checked = true;
        i2iControls.classList.add('active');
    };
    ctxMenu.querySelector('#ctx-copy-img').onclick = async () => {
        ctxMenu.style.display = 'none';
        try {
            await safeInvoke('copy_image_to_clipboard', { path: realPath });
            const n = document.createElement('div');
            n.textContent = 'Image copied to clipboard';
            n.style.cssText = 'position:fixed;bottom:20px;right:20px;background:#2a2a4e;color:#a0d4ff;padding:10px 16px;border-radius:6px;font-size:13px;z-index:9998';
            document.body.appendChild(n);
            setTimeout(() => n.remove(), 2500);
        } catch (err) {
            alert('Copy failed: ' + err);
        }
    };
    ctxMenu.querySelector('#ctx-save').onclick = async () => {
        ctxMenu.style.display = 'none';
        try {
            const dest = await safeInvoke('export_image_to_downloads', { path: realPath });
            const name = dest.split('/').pop();
            const n = document.createElement('div');
            n.textContent = `Saved: ${name}`;
            n.style.cssText = 'position:fixed;bottom:20px;right:20px;background:#2a2a4e;color:#a0d4ff;padding:10px 16px;border-radius:6px;font-size:13px;z-index:9998';
            document.body.appendChild(n);
            setTimeout(() => n.remove(), 3000);
        } catch (err) {
            alert('Export failed: ' + err);
        }
    };
    ctxMenu.querySelector('#ctx-copy-path').onclick = () => {
        ctxMenu.style.display = 'none';
        navigator.clipboard.writeText(realPath).catch(() => {});
    };

    const x = Math.min(e.clientX, window.innerWidth - 180);
    const y = Math.min(e.clientY, window.innerHeight - 80);
    ctxMenu.style.left = x + 'px';
    ctxMenu.style.top = y + 'px';
    ctxMenu.style.display = 'block';
}

const promptInput = document.getElementById('image-prompt');
const generateBtn = document.getElementById('generate-btn');
const statusText = document.getElementById('gen-status');
const resultPreview = document.getElementById('image-result-preview');
const resultImg = document.getElementById('image-result-img');
const gallery = document.getElementById('image-gallery');

// ── img2img state ───────────────────────────────────────────────────────────
let i2iSourcePath = null;
let lastGeneratedPath = null;

const i2iEnabled   = document.getElementById('i2i-enabled');
const i2iControls  = document.getElementById('i2i-controls');
const i2iThumb     = document.getElementById('i2i-thumb');
const i2iName      = document.getElementById('i2i-source-name');
const i2iStrength  = document.getElementById('i2i-strength');
const i2iStrengthVal = document.getElementById('i2i-strength-val');

i2iEnabled.addEventListener('change', () => {
    i2iControls.classList.toggle('active', i2iEnabled.checked);
});

i2iStrength.addEventListener('input', () => {
    i2iStrengthVal.textContent = i2iStrength.value + '%';
});

function setI2ISource(path) {
    i2iSourcePath = path;
    const name = path.split('/').pop();
    i2iName.textContent = name;
    const tauri = getTauri();
    const url = (tauri && tauri.core && tauri.core.convertFileSrc)
        ? tauri.core.convertFileSrc(path) : path;
    i2iThumb.src = url;
    i2iThumb.classList.add('active');
}

document.getElementById('i2i-use-current').addEventListener('click', () => {
    if (lastGeneratedPath) {
        setI2ISource(lastGeneratedPath);
    } else {
        alert('No generated image yet. Generate one first.');
    }
});

document.getElementById('i2i-browse').addEventListener('click', async () => {
    const tauri = getTauri();
    if (!tauri || !tauri.dialog) { alert('Dialog plugin unavailable.'); return; }
    try {
        const path = await tauri.dialog.open({
            multiple: false,
            filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] }]
        });
        if (path && typeof path === 'string') setI2ISource(path);
    } catch (err) {
        console.error('Browse failed:', err);
    }
});

async function loadGallery() {
    console.log("Image Module: Loading gallery...");
    if (!gallery) return;
    try {
        const images = await safeInvoke('list_gallery');
        gallery.innerHTML = '';
        images.forEach(img => {
            const item = document.createElement('div');
            item.className = 'gallery-item';
            const url = getAssetUrl(img.path);
            
            item.innerHTML = `
                <img src="" alt="gen">
                <div class="gallery-item-info"></div>
                <button class="delete-btn" title="Delete image">🗑️</button>
            `;
            item.querySelector('img').src = url;
            item.querySelector('.gallery-item-info').textContent = img.prompt || img.date;
            
            item.querySelector('img').onclick = () => {
                promptInput.value = img.prompt;
                resultImg.src = url;
                resultPreview.classList.add('active');
            };

            item.querySelector('img').addEventListener('contextmenu', (e) => showContextMenu(e, img.path));
            
            item.querySelector('.delete-btn').onclick = async (e) => {
                e.stopPropagation();
                if (confirm("Delete this image?")) {
                    try {
                        await safeInvoke('delete_image', { path: img.path });
                        if (resultImg.src === url) {
                            resultPreview.classList.remove('active');
                        }
                        await loadGallery();
                    } catch (err) {
                        console.error("Failed to delete:", err);
                        alert("Error deleting image: " + err);
                    }
                }
            };
            
            gallery.appendChild(item);
        });
    } catch (err) {
        console.error('Gallery failed:', err);
    }
}

async function generate() {
    console.log("Image Module: Generate clicked");
    const prompt = promptInput.value.trim();
    const engine = document.getElementById('image-engine').value;
    if (!prompt) return;

    generateBtn.disabled = true;
    statusText.textContent = 'Checking ComfyUI...';
    
    try {
        const isRunning = await safeInvoke('check_comfyui');
        if (!isRunning) {
            statusText.textContent = 'Spawning ComfyUI...';
            await safeInvoke('spawn_comfyui');
            await new Promise(r => setTimeout(r, 15000));
        }

        const isI2I = i2iEnabled.checked && i2iSourcePath;
        const strengthVal = isI2I ? parseInt(i2iStrength.value) / 100 : null;
        statusText.textContent = isI2I
            ? `Modifying image (strength ${i2iStrength.value}%)…`
            : 'Generating… (VRAM unloading)';

        const genResult = await safeInvoke('generate_image', {
            prompt,
            engine,
            imagePath: isI2I ? i2iSourcePath : null,
            strength: strengthVal
        });

        statusText.textContent = 'Saving...';
        const imgPath = await safeInvoke('save_generated_image', {
            bytes: genResult.bytes,
            prompt,
            comfyuiSource: genResult.comfyui_path
        });

        lastGeneratedPath = imgPath;
        resultImg.src = getAssetUrl(imgPath);
        resultImg.oncontextmenu = (e) => showContextMenu(e, imgPath);
        resultPreview.classList.add('active');
        
        statusText.textContent = 'Ready';
        await loadGallery();
    } catch (err) {
        console.error("Generation failed:", err);
        statusText.textContent = `Error: ${err}`;
    } finally {
        generateBtn.disabled = false;
    }
}

if (generateBtn) {
    generateBtn.addEventListener('click', generate);
}

// Initial load
setTimeout(loadGallery, 500);

window.refreshGallery = loadGallery;
window.triggerImageGeneration = generate;
