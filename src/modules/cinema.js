(() => {
    const { invoke } = window.__TAURI__.core;
    const { listen } = window.__TAURI__.event;
    
    const videoPrompt = document.getElementById('video-prompt');
    const videoDuration = document.getElementById('video-duration');
    const videoQuality = document.getElementById('video-quality');
    const videoImgBtn = document.getElementById('video-img-btn');
    const videoImgName = document.getElementById('video-img-name');
    const generateBtn = document.getElementById('generate-video-btn');
    const cinemaVideo = document.getElementById('cinema-video');
    const cinemaPoster = document.getElementById('cinema-poster');
    const videoPlaceholder = document.getElementById('video-placeholder');
    const videoGallery = document.getElementById('video-gallery');
    const videoResolution = document.getElementById('video-resolution');
    const videoFps = document.getElementById('video-fps');
    const fpsVal = document.getElementById('fps-val');
    const videoSeed = document.getElementById('video-seed');
    const videoSeedDice = document.getElementById('video-seed-dice');
    const cancelBtn = document.getElementById('cancel-video-btn');

    const assetUrl = (path) => {
        const t = window.__TAURI__;
        if (path.startsWith('http')) return path;
        if (t && t.core && t.core.convertFileSrc) return t.core.convertFileSrc(path);
        return path;
    };

    const gpuFill = document.getElementById('gpu-fill');
    const gpuText = document.getElementById('gpu-text');
    const timeEst = document.getElementById('time-est');

    let selectedImagePath = null;
    let cancelled = false;

    // Update duration estimation
    if (videoDuration) {
        videoDuration.oninput = () => {
            updateEstimation();
        };
    }

    // FPS display
    videoFps.oninput = () => { fpsVal.textContent = videoFps.value; };

    // Randomize seed (visible so a good one can be reused)
    videoSeedDice.onclick = () => { videoSeed.value = Math.floor(Math.random() * 2147483647); };

    // Cancel an in-progress render
    cancelBtn.onclick = () => {
        cancelled = true;
        cancelBtn.textContent = "Cancelling…";
        invoke('interrupt_comfyui').catch(() => {});
    };

    // Live render progress from the ComfyUI websocket (Step value/max)
    listen('video-progress', (e) => {
        const { value, max } = e.payload || {};
        if (max > 0 && generateBtn.disabled && !cancelled) {
            videoPlaceholder.querySelector('span').textContent = `Rendering ${value}/${max}`;
        }
    });

    // Image Picker for I2V
    videoImgBtn.onclick = async () => {
        try {
            const path = await window.__TAURI__.dialog.open({
                multiple: false,
                filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] }]
            });
            if (path) {
                selectedImagePath = path;
                videoImgName.textContent = path.split('/').pop();
                console.log("[Cinema] Selected image for I2V:", path);
            }
        } catch (e) {
            console.error(e);
        }
    };

    async function updateGpuStats() {
        try {
            const stats = await invoke('get_gpu_stats');
            gpuFill.style.width = stats.load + '%';
            gpuText.textContent = `GPU: ${Math.round(stats.load)}%`;
            
            // Adjust bar color based on load
            if (stats.load > 80) gpuFill.style.background = 'var(--orange)';
            else if (stats.load > 50) gpuFill.style.background = 'var(--signal)';
            else gpuFill.style.background = 'var(--carbon)';
            
        } catch (e) {
            // Probably no nvidia-smi
            gpuText.textContent = "GPU: N/A";
        }
    }

    function updateEstimation() {
        const seconds = parseInt(videoDuration.value);
        const quality = videoQuality.value;
        
        let baseTime = quality === 'low' ? 30 : (quality === 'mid' ? 90 : 240);
        let estimatedSeconds = (baseTime * (seconds / 4)).toFixed(0);
        
        const mins = Math.floor(estimatedSeconds / 60);
        const secs = estimatedSeconds % 60;
        timeEst.textContent = `Est: ${mins}:${secs.toString().padStart(2, '0')}`;
    }

    async function generate() {
        const prompt = videoPrompt.value.trim();
        if (!prompt && !selectedImagePath) return alert("Please enter a prompt or select an image.");

        cancelled = false;
        generateBtn.disabled = true;
        generateBtn.textContent = "🎥 Shooting...";
        cancelBtn.style.display = 'block';
        cancelBtn.textContent = "✖ Cancel render";

        try {
            // Start ComfyUI on demand (it stays off when idle), then wait until it's ready.
            const isRunning = await invoke('check_comfyui');
            if (!isRunning) {
                videoPlaceholder.querySelector('span').textContent = "Starting ComfyUI...";
                await invoke('spawn_comfyui');
                let ready = false;
                for (let i = 0; i < 45; i++) {
                    await new Promise(r => setTimeout(r, 2000));
                    if (await invoke('check_comfyui')) { ready = true; break; }
                }
                if (!ready) throw "ComfyUI failed to start within 90s.";
            }
            videoPlaceholder.querySelector('span').textContent = "Developing Film...";

            const res = parseInt(videoResolution.value);
            const seedStr = videoSeed.value.trim();
            const resultPath = await invoke('generate_video', {
                prompt,
                duration: parseInt(videoDuration.value),
                quality: videoQuality.value,
                imagePath: selectedImagePath,
                width: res,
                height: res,
                fps: parseInt(videoFps.value),
                seed: seedStr === '' ? null : parseInt(seedStr)
            });

            console.log("[Cinema] Video ready:", resultPath);

            playVideo(resultPath, resultPath.replace(/\.mp4$/i, '.png'));
            loadVideoGallery();

            alert("Director's Cut Ready!");
        } catch (e) {
            if (cancelled) {
                videoPlaceholder.querySelector('span').textContent = "Render cancelled";
            } else {
                alert("Production Error: " + e);
            }
        } finally {
            generateBtn.disabled = false;
            generateBtn.textContent = "🎬 Action!";
            cancelBtn.style.display = 'none';
            if (!cancelled) videoPlaceholder.querySelector('span').textContent = "Director's Cut Ready";
        }
    }

    generateBtn.onclick = generate;

    function openExternally(videoPath) {
        invoke('open_video', { path: videoPath }).catch(err => alert('Could not open video: ' + err));
    }

    // WebKitGTK can't render <video> reliably on Linux/NVIDIA, so play in the system
    // player and show the poster frame (the VHS .png) in the preview pane.
    function playVideo(videoPath, thumbPath) {
        if (thumbPath) {
            cinemaPoster.src = assetUrl(thumbPath);
            cinemaPoster.style.display = 'block';
        }
        cinemaVideo.style.display = 'none';
        videoPlaceholder.style.display = 'none';
        cinemaPoster.onclick = () => openExternally(videoPath);
        openExternally(videoPath);
    }

    async function loadVideoGallery() {
        if (!videoGallery) return;
        try {
            const videos = await invoke('list_videos');
            videoGallery.innerHTML = '';
            if (!videos.length) {
                videoGallery.innerHTML = '<div style="color: var(--text-dim); font-size: 13px; padding: 8px;">No renders yet. Hit Action! to create one.</div>';
                return;
            }
            videos.forEach(v => {
                const item = document.createElement('div');
                item.className = 'gallery-item';
                item.innerHTML = `
                    <div class="vid-thumb${v.thumb ? '' : ' no-thumb'}">
                        <img alt="render">
                        <div class="play-badge"></div>
                    </div>
                    <div class="gallery-item-info"></div>
                    <button class="delete-btn" title="Delete video">🗑️</button>
                `;
                if (v.thumb) item.querySelector('img').src = assetUrl(v.thumb);
                item.querySelector('.gallery-item-info').textContent = v.date || v.name;
                item.querySelector('.vid-thumb').onclick = () => playVideo(v.path, v.thumb);
                item.querySelector('.delete-btn').onclick = async (e) => {
                    e.stopPropagation();
                    if (confirm('Delete this video?')) {
                        try { await invoke('delete_video', { path: v.path }); loadVideoGallery(); }
                        catch (err) { alert('Delete failed: ' + err); }
                    }
                };
                videoGallery.appendChild(item);
            });
        } catch (e) {
            console.error('[Cinema] gallery load failed', e);
        }
    }

    loadVideoGallery();

    // Triggered by LLM
    listen('llm-done', (event) => {
        const response = event.payload;
        if (response.toLowerCase().includes('generate_video:')) {
            const prompt = response.split(/generate_video:/i)[1].trim();
            videoPrompt.value = prompt;
            window.switchTab('cinema');
        }
    });

    // Polling
    setInterval(updateGpuStats, 3000);
    updateEstimation();
    videoQuality.onchange = updateEstimation;

})();
