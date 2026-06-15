(() => {
    const { invoke } = window.__TAURI__.core;

    const overlay = document.getElementById('inpaint-overlay');
    const closeBtn = document.getElementById('inpaint-close-btn');
    const container = document.getElementById('inpaint-canvas-container');
    const canvas = document.getElementById('inpaint-canvas');
    const maskPreview = document.getElementById('inpaint-mask-preview');
    const maskInteractive = document.getElementById('inpaint-mask-interactive');
    
    const brushSizeSlider = document.getElementById('inpaint-brush-size');
    const brushVal = document.getElementById('inpaint-brush-val');
    const btnPaint = document.getElementById('inpaint-mode-paint');
    const btnErase = document.getElementById('inpaint-mode-erase');
    const btnClear = document.getElementById('inpaint-clear-btn');
    const btnSubmit = document.getElementById('inpaint-submit-btn');
    
    const promptInput = document.getElementById('inpaint-prompt');
    const negativeInput = document.getElementById('inpaint-negative');

    let ctx, maskCtx, interactCtx;
    let isDrawing = false;
    let lastPos = { x: 0, y: 0 };
    let brushSize = 40;
    let mode = 'paint'; // 'paint' or 'erase'
    let currentImagePath = '';
    let scale = 1;

    // A hidden canvas to generate the final mask (white on black)
    const exportMaskCanvas = document.createElement('canvas');
    const exportCtx = exportMaskCanvas.getContext('2d');

    function initEditor(imagePath) {
        currentImagePath = imagePath;
        overlay.classList.remove('hidden');
        
        ctx = canvas.getContext('2d');
        maskCtx = maskPreview.getContext('2d');
        interactCtx = maskInteractive.getContext('2d');

        const img = new Image();
        // Use Tauri asset protocol to load local image
        img.src = window.__TAURI__.core.convertFileSrc(imagePath);
        img.onload = () => {
            // Calculate scale to fit container while maintaining aspect ratio
            const maxWidth = container.clientWidth;
            const maxHeight = window.innerHeight * 0.6; // Max 60% of viewport height
            
            scale = Math.min(maxWidth / img.width, maxHeight / img.height, 1);
            
            const w = img.width * scale;
            const h = img.height * scale;

            container.style.width = `${w}px`;
            container.style.height = `${h}px`;

            [canvas, maskPreview, maskInteractive].forEach(c => {
                c.width = img.width;
                c.height = img.height;
                c.style.width = `${w}px`;
                c.style.height = `${h}px`;
            });

            exportMaskCanvas.width = img.width;
            exportMaskCanvas.height = img.height;

            ctx.drawImage(img, 0, 0);
            
            // Clear mask layers
            maskCtx.clearRect(0, 0, maskPreview.width, maskPreview.height);
            interactCtx.clearRect(0, 0, maskInteractive.width, maskInteractive.height);
            
            // Set styles
            maskCtx.fillStyle = 'rgba(255, 0, 0, 1)';
            interactCtx.lineCap = 'round';
            interactCtx.lineJoin = 'round';
        };
    }

    function closeEditor() {
        overlay.classList.add('hidden');
        promptInput.value = '';
        negativeInput.value = '';
    }

    function getMousePos(e) {
        const rect = maskInteractive.getBoundingClientRect();
        return {
            x: (e.clientX - rect.left) / scale,
            y: (e.clientY - rect.top) / scale
        };
    }

    function startDrawing(e) {
        isDrawing = true;
        lastPos = getMousePos(e);
        draw(e);
    }

    function stopDrawing() {
        if (!isDrawing) return;
        isDrawing = false;
        
        // Transfer interactive strokes to mask preview
        if (mode === 'paint') {
            maskCtx.globalCompositeOperation = 'source-over';
            maskCtx.drawImage(maskInteractive, 0, 0);
        } else {
            maskCtx.globalCompositeOperation = 'destination-out';
            maskCtx.drawImage(maskInteractive, 0, 0);
        }
        
        interactCtx.clearRect(0, 0, maskInteractive.width, maskInteractive.height);
    }

    function draw(e) {
        if (!isDrawing) return;
        
        const pos = getMousePos(e);
        
        interactCtx.lineWidth = brushSize;
        
        if (mode === 'paint') {
            interactCtx.globalCompositeOperation = 'source-over';
            interactCtx.strokeStyle = 'rgba(255, 0, 0, 1)';
        } else {
            // When erasing, we want to see what we're erasing. 
            // We draw white on interactCtx, then composite it with destination-out on stopDrawing.
            interactCtx.globalCompositeOperation = 'source-over';
            interactCtx.strokeStyle = 'rgba(255, 255, 255, 1)';
        }

        interactCtx.beginPath();
        interactCtx.moveTo(lastPos.x, lastPos.y);
        interactCtx.lineTo(pos.x, pos.y);
        interactCtx.stroke();
        
        lastPos = pos;
    }

    function getMaskBase64() {
        exportCtx.fillStyle = 'black';
        exportCtx.fillRect(0, 0, exportMaskCanvas.width, exportMaskCanvas.height);
        
        // Draw the red mask as white
        exportCtx.globalCompositeOperation = 'source-in'; // wait, no
        // Better: iterate pixel data to convert red to white, or use canvas tricks
        // Since maskCtx only has red pixels and transparent pixels:
        // We can draw the maskCtx, then fill white with source-in
        exportCtx.globalCompositeOperation = 'source-over';
        exportCtx.drawImage(maskPreview, 0, 0);
        
        exportCtx.globalCompositeOperation = 'source-in';
        exportCtx.fillStyle = 'white';
        exportCtx.fillRect(0, 0, exportMaskCanvas.width, exportMaskCanvas.height);
        
        exportCtx.globalCompositeOperation = 'destination-over';
        exportCtx.fillStyle = 'black';
        exportCtx.fillRect(0, 0, exportMaskCanvas.width, exportMaskCanvas.height);
        
        return exportMaskCanvas.toDataURL('image/png');
    }

    // Event Listeners
    maskInteractive.addEventListener('mousedown', startDrawing);
    maskInteractive.addEventListener('mousemove', draw);
    window.addEventListener('mouseup', stopDrawing);
    
    brushSizeSlider.addEventListener('input', (e) => {
        brushSize = e.target.value;
        brushVal.textContent = brushSize + 'px';
    });

    btnPaint.addEventListener('click', () => {
        mode = 'paint';
        btnPaint.classList.add('active');
        btnErase.classList.remove('active');
    });

    btnErase.addEventListener('click', () => {
        mode = 'erase';
        btnErase.classList.add('active');
        btnPaint.classList.remove('active');
    });

    btnClear.addEventListener('click', () => {
        maskCtx.clearRect(0, 0, maskPreview.width, maskPreview.height);
        interactCtx.clearRect(0, 0, maskInteractive.width, maskInteractive.height);
    });

    closeBtn.addEventListener('click', closeEditor);

    btnSubmit.addEventListener('click', async () => {
        const prompt = promptInput.value.trim();
        if (!prompt) {
            alert('Please enter a prompt for the inpainting area.');
            return;
        }
        
        btnSubmit.textContent = 'Generating...';
        btnSubmit.disabled = true;
        
        try {
            const maskB64 = getMaskBase64();
            const neg = negativeInput.value.trim();
            
            const result = await invoke('generate_inpainting', {
                imagePath: currentImagePath,
                maskBase64: maskB64,
                prompt: prompt,
                negative: neg ? neg : null
            });
            
            // Use the image module to display the result or switch to it
            if (window.imageGallery && window.imageGallery.addImage) {
                // To be safe, trigger reload
            }
            
            // Switch to image tab and dispatch an event
            document.querySelector('[data-tab="image"]').click();
            window.dispatchEvent(new CustomEvent('image-generated'));
            
            closeEditor();
        } catch (e) {
            alert('Inpainting failed: ' + e);
        } finally {
            btnSubmit.textContent = '🎨 Generate Inpainting';
            btnSubmit.disabled = false;
        }
    });

    // Expose globally
    window.openInpaintEditor = initEditor;
})();
