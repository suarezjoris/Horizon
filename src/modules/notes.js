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

    toggleMindmapBtn.onclick = async () => {
        isMindmapVisible = !isMindmapVisible;
        mindmapContainer.classList.toggle('hidden', !isMindmapVisible);
        noteContent.classList.toggle('hidden', isMindmapVisible);
        if (isMindmapVisible) renderMindmap();
    };

    async function renderMindmap() {
        mindmapContainer.innerHTML = '<div class="loading" style="padding: 20px; color: #b3b3b3; font-family: monospace;">Loading Local Vault Graph...</div>';
        
        try {
            // 1. Locally parse the vault
            const notes = await invoke('list_notes');
            let nodesMap = new Map();
            let links = [];

            // Initialize all nodes
            for (const path of notes) {
                const id = path.replace('.md', '');
                nodesMap.set(id, { id: id, name: id });
            }

            // Read content and extract wikilinks
            for (const path of notes) {
                const sourceId = path.replace('.md', '');
                try {
                    const content = await invoke('read_note', { relPath: path });
                    // Match [[Link]] or [[Link|Alias]]
                    const regex = /\[\[(.*?)\]\]/g;
                    let match;
                    while ((match = regex.exec(content)) !== null) {
                        let target = match[1].split('|')[0].trim();
                        // Ignore internet searches in the graph view
                        if (target.startsWith('internet_search:')) continue;
                        
                        target = target.replace('.md', '');
                        
                        // If target doesn't exist, create it as a ghost node (Obsidian does this)
                        if (!nodesMap.has(target)) {
                            nodesMap.set(target, { id: target, name: target, ghost: true });
                        }

                        links.push({ source: sourceId, target: target });
                    }
                } catch (e) {
                    console.error("Error reading note for graph", e);
                }
            }

            let nodes = Array.from(nodesMap.values());
            
            mindmapContainer.innerHTML = '';
            const width = mindmapContainer.clientWidth;
            const height = mindmapContainer.clientHeight || 500;

            const svg = d3.select("#mindmap-container")
                .append("svg")
                .attr("width", "100%")
                .attr("height", "100%")
                .attr("viewBox", [0, 0, width, height])
                .style("background-color", "transparent"); // Inherit dark theme

            const g = svg.append("g");

            const zoom = d3.zoom()
                .scaleExtent([0.1, 4])
                .on("zoom", (event) => {
                    g.attr("transform", event.transform);
                });

            svg.call(zoom);

            const simulation = d3.forceSimulation(nodes)
                .force("link", d3.forceLink(links).id(d => d.id).distance(100))
                .force("charge", d3.forceManyBody().strength(-300))
                .force("center", d3.forceCenter(width / 2, height / 2))
                .force("x", d3.forceX(width / 2).strength(0.05))
                .force("y", d3.forceY(height / 2).strength(0.05));

            const link = g.append("g")
                .attr("stroke", "rgba(255, 255, 255, 0.2)")
                .selectAll("line")
                .data(links)
                .join("line")
                .attr("stroke-width", 1);

            const node = g.append("g")
                .selectAll("g")
                .data(nodes)
                .join("g")
                .call(d3.drag()
                    .on("start", dragstarted)
                    .on("drag", dragged)
                    .on("end", dragended));

            // Obsidian style: Light grey nodes, slightly smaller, ghost nodes are darker
            node.append("circle")
                .attr("r", 6)
                .attr("fill", d => d.ghost ? "#555555" : "#a8a8a8")
                .attr("stroke", d => d.ghost ? "#333" : "none")
                .attr("stroke-width", 1);

            node.append("text")
                .text(d => d.name)
                .attr("x", 10)
                .attr("y", 4)
                .attr("fill", "rgba(255,255,255,0.7)")
                .style("font-size", "12px")
                .style("font-family", "sans-serif")
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

            // Initial center zoom
            svg.call(zoom.transform, d3.zoomIdentity.translate(width/2, height/2).scale(1).translate(-width/2, -height/2));

        } catch (e) {
            mindmapContainer.innerHTML = `<div class="error" style="color: red; padding: 20px;">Graph failed: ${e}</div>`;
            console.error("Graph error", e);
        }
    }

    window.onNotesTabActive = () => {
        refreshNotes();
        if (isMindmapVisible) renderMindmap();
    };

})();
