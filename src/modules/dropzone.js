(() => {
  const { listen } = window.__TAURI__.event;
  const { invoke } = window.__TAURI__.core;

  class DropZoneManager {
    constructor() {
      this.overlay = this.createOverlay();
      this.badge = this.overlay.querySelector('#dropzone-badge');
      this.text = this.overlay.querySelector('.dropzone-text');
      
      this.handlers = {
        llm: this.handleLLMDrop.bind(this),
        image: this.handleImageDrop.bind(this),
        notes: this.handleNotesDrop.bind(this),
        cinema: this.handleCinemaDrop.bind(this),
      };

      this.initListeners();
    }

    createOverlay() {
      const overlay = document.createElement('div');
      overlay.id = 'dropzone-overlay';
      overlay.className = 'dropzone-overlay';
      // Style will be handled in style.css
      overlay.innerHTML = `
        <div class="dropzone-content">
          <div class="dropzone-icon">📎</div>
          <div class="dropzone-text">Drop to attach file</div>
          <div class="dropzone-badge" id="dropzone-badge" style="display: none;"></div>
        </div>
      `;
      document.body.appendChild(overlay);
      return overlay;
    }

    showOverlay(paths) {
      if (paths && paths.length > 0) {
        const ext = paths[0].split('.').pop().toLowerCase();
        let type = 'File';
        if (['png', 'jpg', 'jpeg', 'webp', 'gif'].includes(ext)) type = 'Image';
        else if (ext === 'pdf') type = 'PDF Document';
        else if (['docx', 'txt', 'md'].includes(ext)) type = 'Text Document';
        else if (['rs', 'js', 'html', 'css', 'py'].includes(ext)) type = 'Code';
        
        this.badge.style.display = 'inline-block';
        this.badge.textContent = type;
        this.text.textContent = paths.length > 1 ? `Drop to attach ${paths.length} files` : 'Drop to attach file';
      } else {
        this.badge.style.display = 'none';
        this.text.textContent = 'Drop to attach file';
      }
      this.overlay.classList.add('active');
    }

    hideOverlay() {
      this.overlay.classList.remove('active');
    }

    initListeners() {
      listen('tauri://file-drop-hover', (e) => {
        this.showOverlay(e.payload);
      });
      listen('tauri://file-drop-cancelled', () => {
        this.hideOverlay();
      });
      listen('tauri://file-drop', (e) => {
        this.hideOverlay();
        const activeTab = document.querySelector('.tab.active')?.dataset.tab;
        if (activeTab && this.handlers[activeTab]) {
          this.handlers[activeTab](e.payload);
        }
      });
      
      // Native drag events just in case
      listen('tauri://drag-enter', (e) => {
         this.showOverlay(e.payload?.paths);
      });
      listen('tauri://drag-leave', () => {
         this.hideOverlay();
      });
      listen('tauri://drop', (e) => {
         this.hideOverlay();
         const activeTab = document.querySelector('.tab.active')?.dataset.tab;
         if (activeTab && this.handlers[activeTab]) {
             this.handlers[activeTab](e.payload?.paths || []);
         }
      });
    }

    async handleLLMDrop(files) {
      if (window.handleLLMDropFiles) {
        window.handleLLMDropFiles(files);
      }
    }

    async handleImageDrop(files) {
      if (!files || files.length === 0) return;
      const ext = files[0].split('.').pop().toLowerCase();
      if (['png', 'jpg', 'jpeg', 'webp'].includes(ext)) {
         if (window.setI2ISource) {
             window.setI2ISource(files[0]);
         }
      } else {
          alert('Please drop a valid image file for Img2Img.');
      }
    }

    async handleNotesDrop(files) {
       if (!files || files.length === 0) return;
       const path = files[0];
       try {
          const content = await invoke('read_file_content', { path });
          const textContent = content.data || content;
          if (window.importNoteFromDrop) {
             window.importNoteFromDrop(path, textContent);
          } else {
             const filename = path.split('/').pop().split('\\').pop();
             const cleanName = filename.endsWith('.md') ? filename : filename + '.md';
             await invoke('write_note', { relPath: `imports/${cleanName}`, content: textContent });
             alert(`Imported ${filename} to vault.`);
             if (window.onNotesTabActive) window.onNotesTabActive();
          }
       } catch(e) {
          alert('Failed to import note: ' + e);
       }
    }

    async handleCinemaDrop(files) {
      if (!files || files.length === 0) return;
      const ext = files[0].split('.').pop().toLowerCase();
      if (['png', 'jpg', 'jpeg', 'webp'].includes(ext)) {
         // Create UI element if it doesn't exist
         let preview = document.getElementById('cinema-base-preview');
         if (!preview) {
             preview = document.createElement('img');
             preview.id = 'cinema-base-preview';
             preview.style.cssText = "max-height: 100px; max-width: 100px; margin-top: 10px; border-radius: 4px; border: var(--border-clay);";
             const inputBox = document.getElementById('cinema-input-box');
             if (inputBox) {
                 inputBox.appendChild(preview);
             }
         }
         
         const tauri = window.__TAURI__;
         const url = (tauri && tauri.core && tauri.core.convertFileSrc)
             ? tauri.core.convertFileSrc(files[0])
             : `file://${files[0]}`;
         preview.src = url;
         preview.style.display = 'block';
         preview.dataset.path = files[0];
         window.cinemaBaseImagePath = files[0]; // Export for cinema.js to pick up
      } else {
          alert('Please drop an image file for Cinema img2vid.');
      }
    }
  }

  window.addEventListener('DOMContentLoaded', () => {
    window.dropZoneManager = new DropZoneManager();
  });
})();
