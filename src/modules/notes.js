(() => {
    const { invoke } = window.__TAURI__.core;
    
    const notesList = document.getElementById('notes-list');
    const noteTitle = document.getElementById('note-title');
    const noteContent = document.getElementById('note-content');
    const saveBtn = document.getElementById('save-note-btn');
    const newNoteBtn = document.getElementById('new-note-btn');

    let currentNotePath = null;

    async function refreshNotes() {
        try {
            const notes = await invoke('list_notes');
            notesList.innerHTML = '';
            notes.forEach(path => {
                const item = document.createElement('div');
                item.className = 'note-item';
                if (path === currentNotePath) item.classList.add('active');
                item.textContent = path.replace('.md', '');
                item.onclick = () => loadNote(path);
                notesList.appendChild(item);
            });
        } catch (e) {
            console.error("Failed to list notes", e);
        }
    }

    async function loadNote(path) {
        try {
            const content = await invoke('read_note', { relPath: path });
            currentNotePath = path;
            noteTitle.value = path.replace('.md', '');
            noteContent.value = content;
            refreshNotes();
        } catch (e) {
            alert("Error reading note: " + e);
        }
    }

    saveBtn.onclick = async () => {
        const title = noteTitle.value.trim();
        const content = noteContent.value;
        if (!title) return alert("Please enter a title");

        const path = title.endsWith('.md') ? title : `${title}.md`;
        try {
            await invoke('write_note', { relPath: path, content });
            currentNotePath = path;
            refreshNotes();
            alert("Note saved!");
        } catch (e) {
            alert("Error saving note: " + e);
        }
    };

    newNoteBtn.onclick = () => {
        currentNotePath = null;
        noteTitle.value = '';
        noteContent.value = '';
        refreshNotes();
    };

    window.onNotesTabActive = () => {
        refreshNotes();
    };

})();
