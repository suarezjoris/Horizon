const commands = [
    { name: '/analyze-topics', desc: 'Proposer un nouveau hub pour les notes non catégorisées' },
    { name: '/clear', desc: 'Effacer la conversation' },
    { name: '/consolidate', desc: 'Consolider les neurones et refactoriser le Second Cerveau' },
    { name: '/docx', desc: 'Générer un document Word' },
    { name: '/excel', desc: 'Générer un fichier Excel' },
    { name: '/learn_wiki', desc: 'Scanner Wikipédia pour ingérer des données' },
    { name: '/memory', desc: 'Lire la mémoire système' },
    { name: '/powerpoint', desc: 'Générer un PowerPoint' },
    { name: '/ppt', desc: 'Générer un PowerPoint' },
    { name: '/pptx', desc: 'Générer un PowerPoint' },
    { name: '/reindex', desc: 'Forcer la réindexation complète du Vault' },
    { name: '/remember', desc: 'Sauvegarder la dernière réponse dans une note' },
    { name: '/save', desc: 'Sauvegarder la dernière réponse dans une note' },
    { name: '/search', desc: 'Chercher dans la mémoire' },
    { name: '/topics', desc: 'Analyser la santé des hubs (topics) du Vault' },
    { name: '/wiki', desc: 'Scanner Wikipédia pour ingérer des données' },
    { name: '/word', desc: 'Générer un document Word' },
    { name: '/xlsx', desc: 'Générer un fichier Excel' }
];

// Binary search implementation (Recherche dichotomique)
function findCommands(prefix) {
    if (!prefix.startsWith('/')) return [];
    
    // Commands array is already sorted alphabetically by name
    let low = 0;
    let high = commands.length - 1;
    let firstMatch = -1;
    
    while (low <= high) {
        let mid = Math.floor((low + high) / 2);
        let cmd = commands[mid].name;
        
        if (cmd.startsWith(prefix)) {
            firstMatch = mid;
            // Continue searching left to find the absolute first match
            high = mid - 1;
        } else if (cmd < prefix) {
            low = mid + 1;
        } else {
            high = mid - 1;
        }
    }
    
    if (firstMatch === -1) return [];
    
    // Collect all matches
    let results = [];
    for (let i = firstMatch; i < commands.length; i++) {
        if (commands[i].name.startsWith(prefix)) {
            results.push(commands[i]);
        } else {
            break; // Stop since it's sorted
        }
    }
    
    return results;
}

document.addEventListener('DOMContentLoaded', () => {
    const chatInput = document.getElementById('chat-input');
    const popup = document.getElementById('slash-commands-popup');
    let selectedIndex = 0;
    let currentMatches = [];
    let isPopupOpen = false;

    function renderPopup() {
        popup.innerHTML = '';
        if (currentMatches.length === 0) {
            popup.classList.add('hidden');
            isPopupOpen = false;
            return;
        }

        currentMatches.forEach((cmd, index) => {
            const div = document.createElement('div');
            div.className = 'slash-command-item' + (index === selectedIndex ? ' selected' : '');
            div.innerHTML = `<div class="slash-command-name">${cmd.name}</div><div class="slash-command-desc">${cmd.desc}</div>`;
            
            div.addEventListener('click', () => {
                insertCommand(cmd.name);
            });
            
            popup.appendChild(div);
        });
        
        popup.classList.remove('hidden');
        isPopupOpen = true;

        // Ensure the selected item is visible and resets scroll properly
        const selectedEl = popup.children[selectedIndex];
        if (selectedEl) {
            selectedEl.scrollIntoView({ block: 'nearest' });
        }
    }

    function insertCommand(cmdName) {
        const words = chatInput.value.split(' ');
        words.pop(); // Remove the current typing word
        words.push(cmdName + ' ');
        chatInput.value = words.join(' ');
        chatInput.focus();
        
        popup.classList.add('hidden');
        isPopupOpen = false;
    }

    chatInput.addEventListener('input', (e) => {
        const text = chatInput.value;
        const words = text.split(' ');
        const lastWord = words[words.length - 1];

        if (lastWord.startsWith('/')) {
            currentMatches = findCommands(lastWord);
            selectedIndex = 0;
            renderPopup();
        } else {
            popup.classList.add('hidden');
            isPopupOpen = false;
        }
    });

    chatInput.addEventListener('keydown', (e) => {
        if (!isPopupOpen) return;

        if (e.key === 'ArrowDown') {
            e.preventDefault();
            selectedIndex = (selectedIndex + 1) % currentMatches.length;
            renderPopup();
        } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            selectedIndex = (selectedIndex - 1 + currentMatches.length) % currentMatches.length;
            renderPopup();
        } else if (e.key === 'Enter' || e.key === 'Tab') {
            e.preventDefault();
            if (currentMatches[selectedIndex]) {
                insertCommand(currentMatches[selectedIndex].name);
            }
        } else if (e.key === 'Escape') {
            popup.classList.add('hidden');
            isPopupOpen = false;
        }
    });
    
    // Close popup if clicking outside
    document.addEventListener('click', (e) => {
        if (isPopupOpen && e.target !== chatInput && !popup.contains(e.target)) {
            popup.classList.add('hidden');
            isPopupOpen = false;
        }
    });
});
