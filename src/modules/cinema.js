(() => {
    const { invoke } = window.__TAURI__.core;
    const { listen } = window.__TAURI__.event;
    
    const videoPrompt = document.getElementById('video-prompt');
    const videoDuration = document.getElementById('video-duration');
    const durationVal = document.getElementById('duration-val');
    const videoQuality = document.getElementById('video-quality');
    const videoImgBtn = document.getElementById('video-img-btn');
    const videoImgName = document.getElementById('video-img-name');
    const generateBtn = document.getElementById('generate-video-btn');
    const cinemaVideo = document.getElementById('cinema-video');
    const videoPlaceholder = document.getElementById('video-placeholder');
    
    const gpuFill = document.getElementById('gpu-fill');
    const gpuText = document.getElementById('gpu-text');
    const timeEst = document.getElementById('time-est');

    let selectedImagePath = null;

    // Update duration display
    videoDuration.oninput = () => {
        durationVal.textContent = videoDuration.value + 's';
        updateEstimation();
    };

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
            if (stats.load > 80) gpuFill.style.background = '#ff5f57';
            else if (stats.load > 50) gpuFill.style.background = 'var(--accent-gold)';
            else gpuFill.style.background = 'var(--accent-teal)';
            
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

        generateBtn.disabled = true;
        generateBtn.textContent = "🎥 Shooting...";
        videoPlaceholder.querySelector('span').textContent = "Developing Film...";

        try {
            const resultPath = await invoke('generate_video', {
                prompt,
                duration: parseInt(videoDuration.value),
                quality: videoQuality.value,
                imagePath: selectedImagePath
            });

            console.log("[Cinema] Video ready:", resultPath);
            
            // Show video (assuming the backend returns a URL or we use asset://)
            cinemaVideo.src = resultPath.startsWith('http') ? resultPath : `asset://localhost${resultPath}`;
            cinemaVideo.style.display = 'block';
            videoPlaceholder.style.display = 'none';
            
            alert("Director's Cut Ready!");
        } catch (e) {
            alert("Production Error: " + e);
        } finally {
            generateBtn.disabled = false;
            generateBtn.textContent = "🎬 Action!";
            videoPlaceholder.querySelector('span').textContent = "Director's Cut Ready";
        }
    }

    generateBtn.onclick = generate;

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
