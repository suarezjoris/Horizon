import sys

content = open('src/modules/notes.js').read()
new_load_note = """    async function loadNote(path) {
        try {
            const content = await invoke('read_note', { relPath: path });
            currentNotePath = path;
            noteTitle.value = path.replace('.md', '');
            noteContent.value = content;
            
            try {
                const stats = await invoke('get_note_decay_stats', { relPath: path });
                const statsEl = document.getElementById('note-decay-stats');
                if (stats.status === "not_indexed") {
                    statsEl.textContent = "Not indexed";
                } else {
                    statsEl.textContent = `Scoring: Decay ${stats.decay_factor}x | Boost ${stats.boost_factor}x | Total Mult ${stats.current_multiplier}x | Access: ${stats.total_access} | Days: ${stats.days_since_access} | ${stats.pinned ? '[PINNED]' : ''}`;
                }
            } catch (e) {
                console.error("Failed to load decay stats", e);
            }
            
            refreshNotes();
        } catch (e) {
            alert("Error reading note: " + e);
        }
    }"""

content = content.replace("""    async function loadNote(path) {
        try {
            const content = await invoke('read_note', { relPath: path });
            currentNotePath = path;
            noteTitle.value = path.replace('.md', '');
            noteContent.value = content;
            refreshNotes();
        } catch (e) {
            alert("Error reading note: " + e);
        }
    }""", new_load_note)

open('src/modules/notes.js', 'w').write(content)
