(() => {
    const { invoke } = window.__TAURI__.core;
    
    const notesList = document.getElementById('notes-list');
    const noteTitle = document.getElementById('note-title');
    const noteContent = document.getElementById('note-content');
    const saveBtn = document.getElementById('save-note-btn');
    const newNoteBtn = document.getElementById('new-note-btn');
    const toggleMindmapBtn = document.getElementById('toggle-mindmap-btn');
    const mindmapContainer = document.getElementById('mindmap-container');

    let currentNotePath = null;
    let isMindmapVisible = false;

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
            if (isMindmapVisible) renderMindmap();
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

    toggleMindmapBtn.onclick = () => {
        isMindmapVisible = !isMindmapVisible;
        mindmapContainer.classList.toggle('hidden', !isMindmapVisible);
        noteContent.classList.toggle('hidden', isMindmapVisible);
        if (isMindmapVisible) renderMindmap();
    };

    async function renderMindmap() {
        console.log("[NeuralField] Rendering graph...");
        mindmapContainer.innerHTML = '';
        
        try {
            // Fetch ALL notes from the sidebar to build the graph
            const notes = await invoke('list_notes');
            let nodes = [{ id: "Horizon", group: 0 }];
            let links = [];

            for (const path of notes) {
                try {
                    const content = await invoke('read_note', { relPath: path });
                    const fileName = path.replace('.md', '');
                    nodes.push({ id: path, name: fileName, group: 1 });
                    links.push({ source: "Horizon", target: path });

                    // Extract bullet points (* or -) as sub-nodes
                    const lines = content.split('\n').filter(l => {
                        const trimmed = l.trim();
                        return trimmed.startsWith('-') || trimmed.startsWith('*');
                    });

                    lines.forEach((l, i) => {
                        const factId = `${path}_fact_${i}`;
                        const factText = l.trim().substring(1).trim();
                        if (factText.length > 3) {
                            nodes.push({ id: factId, name: factText, group: 2 });
                            links.push({ source: path, target: factId });
                        }
                    });
                } catch (e) {}
            }

            const width = mindmapContainer.clientWidth;
            const height = mindmapContainer.clientHeight || 500;

            const svg = d3.select("#mindmap-container")
                .append("svg")
                .attr("width", "100%")
                .attr("height", "100%")
                .attr("viewBox", [0, 0, width, height]);

            const simulation = d3.forceSimulation(nodes)
                .force("link", d3.forceLink(links).id(d => d.id).distance(100))
                .force("charge", d3.forceManyBody().strength(-200))
                .force("center", d3.forceCenter(width / 2, height / 2));

            const link = svg.append("g")
                .attr("stroke", "rgba(212, 175, 55, 0.2)")
                .selectAll("line")
                .data(links)
                .join("line");

            const node = svg.append("g")
                .selectAll("g")
                .data(nodes)
                .join("g")
                .call(d3.drag()
                    .on("start", dragstarted)
                    .on("drag", dragged)
                    .on("end", dragended));

            node.append("circle")
                .attr("r", d => d.group === 0 ? 12 : d.group === 1 ? 8 : 5)
                .attr("fill", d => d.group === 0 ? "#d4af37" : d.group === 1 ? "#00f2ff" : "#fff")
                .attr("filter", "drop-shadow(0 0 5px rgba(212, 175, 55, 0.5))");

            node.append("text")
                .text(d => d.name || d.id)
                .attr("x", 12)
                .attr("y", 4)
                .attr("fill", "rgba(255,255,255,0.7)")
                .style("font-size", "10px")
                .style("pointer-events", "none");

            simulation.on("tick", () => {
                link.attr("x1", d => d.source.x)
                    .attr("y1", d => d.source.y)
                    .attr("x2", d => d.target.x)
                    .attr("y2", d => d.target.y);

                node.attr("transform", d => `translate(${d.x},${d.y})`);
            });

            function dragstarted(event) {
                if (!event.active) simulation.alphaTarget(0.3).restart();
                event.subject.fx = event.subject.x;
                event.subject.fy = event.subject.y;
            }
            function dragged(event) {
                event.subject.fx = event.x;
                event.subject.fy = event.y;
            }
            function dragended(event) {
                if (!event.active) simulation.alphaTarget(0);
                event.subject.fx = null;
                event.subject.fy = null;
            }

        } catch (e) {
            console.error("Graph error", e);
        }
    }

    window.onNotesTabActive = () => {
        refreshNotes();
        if (isMindmapVisible) renderMindmap();
    };

})();
