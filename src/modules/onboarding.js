(() => {
    const { invoke } = window.__TAURI__.core;
    
    const overlay = document.getElementById('onboarding-overlay');
    const input = document.getElementById('onboarding-input');
    const submitBtn = document.getElementById('submit-calibration-btn');

    async function checkFirstRun() {
        try {
            const userProfile = await invoke('read_note', { relPath: 'memory/user.md' });
            // If it's just the header, it's a first run
            if (userProfile.trim() === "# User Profile" || userProfile.trim() === "") {
                overlay.classList.remove('hidden');
            }
        } catch (e) {
            // If file doesn't exist, it's a first run
            overlay.classList.remove('hidden');
        }
    }

    submitBtn.onclick = async () => {
        const text = input.value.trim();
        if (!text) return alert("Please share something about yourself first.");

        submitBtn.disabled = true;
        submitBtn.textContent = "Weaving Archetype...";

        try {
            await invoke('process_calibration', { text });
            await invoke('reindex');
            
            overlay.style.opacity = '0';
            setTimeout(() => overlay.classList.add('hidden'), 500);
            alert("Calibration successful. Your personalized latent space is ready.");
        } catch (e) {
            alert("Calibration failed: " + e);
            submitBtn.disabled = false;
            submitBtn.textContent = "Calibrate Horizon";
        }
    };

    // Delay a bit to ensure diagnostic is done
    setTimeout(checkFirstRun, 2000);

})();
