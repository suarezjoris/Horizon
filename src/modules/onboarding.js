(() => {
    const { invoke } = window.__TAURI__.core;
    
    const overlay = document.getElementById('onboarding-overlay');
    const input = document.getElementById('onboarding-input');
    const submitBtn = document.getElementById('submit-calibration-btn');

    async function checkFirstRun() {
        try {
            const notes = await invoke('list_notes');
            if (notes.length === 0) {
                overlay.classList.remove('hidden');
                return;
            }

            // If the user has custom notes from the emergent brain, it's definitely not a first run.
            const hasCustomNotes = notes.some(n => !['memory/user.md', 'memory/code.md', 'memory/skills.md'].includes(n));
            if (hasCustomNotes) {
                return; // Not a first run
            }

            // If only default notes exist, check if user.md was modified
            try {
                const userProfile = await invoke('read_note', { relPath: 'memory/user.md' });
                if (userProfile.trim() === "# User Profile" || userProfile.trim() === "") {
                    overlay.classList.remove('hidden');
                }
            } catch (err) {
                // If user.md doesn't exist but other notes do, it's not a first run
            }
        } catch (e) {
            console.error("Error checking first run status:", e);
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
