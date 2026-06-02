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
    // Fallback for older v2 beta or different configs
    if (tauri && tauri.tauri && tauri.tauri.convertFileSrc) {
        return tauri.tauri.convertFileSrc(path);
    }
    return path;
}

const promptInput = document.getElementById('image-prompt');
const generateBtn = document.getElementById('generate-btn');
const statusText = document.getElementById('gen-status');
const resultPreview = document.getElementById('image-result-preview');
const resultImg = document.getElementById('image-result-img');
const gallery = document.getElementById('image-gallery');

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

        statusText.textContent = 'Generating... (VRAM unloading)';
        const bytes = await safeInvoke('generate_image', { prompt });
        
        statusText.textContent = 'Saving...';
        const imgPath = await safeInvoke('save_generated_image', { bytes, prompt });
        
        resultImg.src = getAssetUrl(imgPath);
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
