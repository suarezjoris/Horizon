# Graph Report - Horizon  (2026-06-03)

## Corpus Check
- 54 files · ~199,856 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1200 nodes · 2857 edges · 82 communities (68 shown, 14 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 17 edges (avg confidence: 0.73)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- [[_COMMUNITY_Community 0|Community 0]]
- [[_COMMUNITY_Community 1|Community 1]]
- [[_COMMUNITY_Community 2|Community 2]]
- [[_COMMUNITY_Community 3|Community 3]]
- [[_COMMUNITY_Community 4|Community 4]]
- [[_COMMUNITY_Community 5|Community 5]]
- [[_COMMUNITY_Community 6|Community 6]]
- [[_COMMUNITY_Community 7|Community 7]]
- [[_COMMUNITY_Community 8|Community 8]]
- [[_COMMUNITY_Community 9|Community 9]]
- [[_COMMUNITY_Community 10|Community 10]]
- [[_COMMUNITY_Community 11|Community 11]]
- [[_COMMUNITY_Community 12|Community 12]]
- [[_COMMUNITY_Community 13|Community 13]]
- [[_COMMUNITY_Community 14|Community 14]]
- [[_COMMUNITY_Community 15|Community 15]]
- [[_COMMUNITY_Community 16|Community 16]]
- [[_COMMUNITY_Community 17|Community 17]]
- [[_COMMUNITY_Community 18|Community 18]]
- [[_COMMUNITY_Community 19|Community 19]]
- [[_COMMUNITY_Community 20|Community 20]]
- [[_COMMUNITY_Community 21|Community 21]]
- [[_COMMUNITY_Community 22|Community 22]]
- [[_COMMUNITY_Community 24|Community 24]]
- [[_COMMUNITY_Community 25|Community 25]]
- [[_COMMUNITY_Community 26|Community 26]]
- [[_COMMUNITY_Community 27|Community 27]]
- [[_COMMUNITY_Community 28|Community 28]]
- [[_COMMUNITY_Community 29|Community 29]]
- [[_COMMUNITY_Community 30|Community 30]]
- [[_COMMUNITY_Community 31|Community 31]]
- [[_COMMUNITY_Community 32|Community 32]]
- [[_COMMUNITY_Community 33|Community 33]]
- [[_COMMUNITY_Community 34|Community 34]]
- [[_COMMUNITY_Community 35|Community 35]]
- [[_COMMUNITY_Community 36|Community 36]]
- [[_COMMUNITY_Community 37|Community 37]]
- [[_COMMUNITY_Community 38|Community 38]]
- [[_COMMUNITY_Community 40|Community 40]]
- [[_COMMUNITY_Community 41|Community 41]]
- [[_COMMUNITY_Community 42|Community 42]]
- [[_COMMUNITY_Community 43|Community 43]]
- [[_COMMUNITY_Community 44|Community 44]]
- [[_COMMUNITY_Community 45|Community 45]]
- [[_COMMUNITY_Community 46|Community 46]]
- [[_COMMUNITY_Community 47|Community 47]]
- [[_COMMUNITY_Community 51|Community 51]]
- [[_COMMUNITY_Community 52|Community 52]]
- [[_COMMUNITY_Community 53|Community 53]]
- [[_COMMUNITY_Community 54|Community 54]]
- [[_COMMUNITY_Community 55|Community 55]]
- [[_COMMUNITY_Community 56|Community 56]]
- [[_COMMUNITY_Community 57|Community 57]]
- [[_COMMUNITY_Community 58|Community 58]]
- [[_COMMUNITY_Community 59|Community 59]]
- [[_COMMUNITY_Community 60|Community 60]]
- [[_COMMUNITY_Community 61|Community 61]]
- [[_COMMUNITY_Community 62|Community 62]]
- [[_COMMUNITY_Community 63|Community 63]]
- [[_COMMUNITY_Community 64|Community 64]]
- [[_COMMUNITY_Community 70|Community 70]]
- [[_COMMUNITY_Community 71|Community 71]]
- [[_COMMUNITY_Community 72|Community 72]]
- [[_COMMUNITY_Community 73|Community 73]]
- [[_COMMUNITY_Community 74|Community 74]]
- [[_COMMUNITY_Community 75|Community 75]]
- [[_COMMUNITY_Community 76|Community 76]]
- [[_COMMUNITY_Community 78|Community 78]]
- [[_COMMUNITY_Community 79|Community 79]]

## God Nodes (most connected - your core abstractions)
1. `E` - 120 edges
2. `d` - 97 edges
3. `constructor()` - 79 edges
4. `P` - 68 edges
5. `fire()` - 67 edges
6. `get()` - 55 edges
7. `s()` - 50 edges
8. `i()` - 49 edges
9. `c()` - 49 edges
10. `n()` - 40 edges

## Surprising Connections (you probably didn't know these)
- `Horizon` --references--> `icon`  [EXTRACTED]
  src/index.html → src-tauri/icons/icon.png
- `main()` --calls--> `PersonalAI`  [EXTRACTED]
  main.py → tui/app.py
- `textual` --references--> `Horizon`  [EXTRACTED]
  requirements.txt → src/index.html
- `chromadb` --references--> `Horizon`  [EXTRACTED]
  requirements.txt → src/index.html
- `httpx` --references--> `Horizon`  [EXTRACTED]
  requirements.txt → src/index.html

## Import Cycles
- 1-file cycle: `src-tauri/src/audio.rs -> src-tauri/src/audio.rs`
- 1-file cycle: `src-tauri/src/vault.rs -> src-tauri/src/vault.rs`
- 1-file cycle: `src-tauri/src/openclaude.rs -> src-tauri/src/openclaude.rs`
- 1-file cycle: `src-tauri/src/settings.rs -> src-tauri/src/settings.rs`

## Communities (82 total, 14 thin omitted)

### Community 0 - "Community 0"
Cohesion: 0.23
Nodes (4): getBlankLine(), getNullCell(), markDirty(), markRangeDirty()

### Community 1 - "Community 1"
Cohesion: 0.04
Nodes (19): _announceCharacters(), _bubbleScroll(), _clearLiveRegion(), compositionupdate(), _createSelectionElement(), getCell(), getStringCellWidth(), _handleChar() (+11 more)

### Community 2 - "Community 2"
Cohesion: 0.08
Nodes (7): _addStyle(), _applyMinimumContrast(), createRow(), _getContrastCache(), i(), setColor(), v()

### Community 3 - "Community 3"
Cohesion: 0.10
Nodes (18): ComposeResult, int, main(), app, security, windows, withGlobalTauri, enable (+10 more)

### Community 4 - "Community 4"
Cohesion: 0.08
Nodes (41): 3, class_type, inputs, 4, class_type, inputs, 5, class_type (+33 more)

### Community 5 - "Community 5"
Cohesion: 0.07
Nodes (4): E, nextStop(), setgCharset(), setgLevel()

### Community 6 - "Community 6"
Cohesion: 0.08
Nodes (41): 3, class_type, inputs, 4, class_type, inputs, 5, class_type (+33 more)

### Community 7 - "Community 7"
Cohesion: 0.15
Nodes (24): Path, read_file_content(), Result, String, PathBuf, Result, String, Vec (+16 more)

### Community 8 - "Community 8"
Cohesion: 0.09
Nodes (20): addEncoding(), addProtocol(), addRefreshCallback(), attachToDom(), buffer(), clearRange(), constructor(), createInstance() (+12 more)

### Community 9 - "Community 9"
Cohesion: 0.10
Nodes (8): addCsiHandler(), addDcsHandler(), c(), clearHandler(), registerCsiHandler(), registerDcsHandler(), registerHandler(), setHandlerFallback()

### Community 10 - "Community 10"
Cohesion: 0.14
Nodes (19): Arc, Mutex, chat(), search_vault(), OpenClaudeState, send_openclaude_raw(), start_openclaude(), AppHandle (+11 more)

### Community 12 - "Community 12"
Cohesion: 0.09
Nodes (8): compositionend(), _finalizeComposition(), _handleAnyTextareaChanges(), hasSelection(), keydown(), P, shouldColumnSelect(), triggerDataEvent()

### Community 13 - "Community 13"
Cohesion: 0.19
Nodes (17): gallery, generate(), generateBtn, getAssetUrl(), getTauri(), loadGallery(), promptInput, resultImg (+9 more)

### Community 14 - "Community 14"
Cohesion: 0.28
Nodes (3): a(), compositionstart(), handleFocus()

### Community 15 - "Community 15"
Cohesion: 0.09
Nodes (5): addEscHandler(), addOscHandler(), registerEscHandler(), registerOscHandler(), s()

### Community 16 - "Community 16"
Cohesion: 0.19
Nodes (7): end(), hook(), put(), reset(), _start(), unhook(), values()

### Community 17 - "Community 17"
Cohesion: 0.20
Nodes (11): _addMouseDownListeners(), _areCoordsInSelection(), _getMouseBufferCoords(), _handleDoubleClick(), _handleIncrementalClick(), _handleMouseDown(), _isCellInSelection(), _isClickInSelection() (+3 more)

### Community 18 - "Community 18"
Cohesion: 0.06
Nodes (27): _cancelCallback(), clear(), clearListeners(), fillViewportRows(), _fireOnCanvasResize(), flush(), _getCorrectBufferLength(), handleCharSizeChanged() (+19 more)

### Community 19 - "Community 19"
Cohesion: 0.26
Nodes (16): ChunkMessage, chat_once(), chat_stream(), ChatChunk, ChatRequest, ChunkMessage, describe_image(), embed() (+8 more)

### Community 20 - "Community 20"
Cohesion: 0.25
Nodes (12): Default, config_path(), get_settings(), load(), persist(), save_settings(), Settings, settings_roundtrip_via_json() (+4 more)

### Community 21 - "Community 21"
Cohesion: 0.13
Nodes (20): clearSelection(), deregister(), disable(), _dragScroll(), _fireEventIfSelectionChanged(), _fireOnSelectionChange(), _getMouseEventScrollAmount(), getWrappedRangeForLine() (+12 more)

### Community 22 - "Community 22"
Cohesion: 0.08
Nodes (11): addLineToLink(), addMarker(), _getEntryIdKey(), _handleBoundaryFocus(), length(), loadAddon(), n(), _refreshRowElements() (+3 more)

### Community 24 - "Community 24"
Cohesion: 0.24
Nodes (11): _clearCurrentLink(), _createLinkUnderlineEvent(), _fireUnderlineEvent(), getCoords(), _handleHover(), _handleMouseUp(), _handleNewLink(), _linkAtPosition() (+3 more)

### Community 25 - "Community 25"
Cohesion: 0.24
Nodes (12): _make_ollama_response(), _make_stream_chunks(), str, Mock d'une réponse Ollama non-streamée., Vérifie que le loop s'arrête après 5 itérations même sans réponse finale., Mock d'un stream Ollama token par token., test_run_agent_calls_fetch_url(), test_run_agent_calls_web_search() (+4 more)

### Community 26 - "Community 26"
Cohesion: 0.07
Nodes (50): A(), autolink(), blockquote(), blockTokens(), br(), checkbox(), code(), codespan() (+42 more)

### Community 27 - "Community 27"
Cohesion: 0.24
Nodes (10): check_comfyui(), ComfyManager, generate_image(), spawn_comfyui(), Child, Option, Result, Self (+2 more)

### Community 28 - "Community 28"
Cohesion: 0.42
Nodes (11): Character, ChatMessage, clear_roleplay_chat(), get_chat_history(), import_character_card(), list_characters(), send_roleplay_message(), AppHandle (+3 more)

### Community 29 - "Community 29"
Cohesion: 0.18
Nodes (11): Code Tab User Guide, chromadb, duckduckgo-search, html2text, httpx, ollama, pytest, pytest-mock (+3 more)

### Community 30 - "Community 30"
Cohesion: 0.40
Nodes (5): _refreshCanvasDimensions(), _refreshColorZonePadding(), _refreshDrawConstants(), _refreshDrawHeightConstants(), setPadding()

### Community 31 - "Community 31"
Cohesion: 0.17
Nodes (9): S(), delete(), f(), forEachByKey(), getKeyIterator(), insert(), _removeMarkerFromLink(), _search() (+1 more)

### Community 32 - "Community 32"
Cohesion: 0.08
Nodes (17): activeProtocol(), activeVersion(), b(), clearAllMarkers(), clearMarkers(), debug(), dispose(), _equalEvents() (+9 more)

### Community 33 - "Community 33"
Cohesion: 0.12
Nodes (5): decode(), h(), _reflowSmaller(), scroll(), translateToString()

### Community 34 - "Community 34"
Cohesion: 0.33
Nodes (11): chunk_text(), cosine_similarity(), Entry, load_index(), reindex(), save_index(), search(), Arc (+3 more)

### Community 35 - "Community 35"
Cohesion: 0.16
Nodes (5): _clearSmoothScrollState(), handleWheel(), scrollLines(), _smoothScroll(), _smoothScrollPercent()

### Community 36 - "Community 36"
Cohesion: 0.32
Nodes (5): _convertViewportColToCharacterIndex(), getJoinedCharacters(), _getWordAt(), _isCharWordSeparator(), _stringRangesToCellRanges()

### Community 37 - "Community 37"
Cohesion: 0.50
Nodes (4): _createElement(), _doRefreshDecorations(), _refreshXPosition(), _renderDecoration()

### Community 38 - "Community 38"
Cohesion: 0.50
Nodes (7): delete_image(), GalleryImage, list_gallery(), save_generated_image(), Result, String, Vec

### Community 40 - "Community 40"
Cohesion: 0.20
Nodes (9): Code Tab User Guide, Example Tasks, Keyboard Shortcuts, Opening a Project, Running Code, Tips, Using the AI Agent, Using the Code Editor (+1 more)

### Community 41 - "Community 41"
Cohesion: 0.25
Nodes (8): error(), _evalLazyOptionalParams(), _getJoinedRanges(), info(), _log(), _mergeRanges(), trace(), warn()

### Community 42 - "Community 42"
Cohesion: 0.18
Nodes (11): addBubble(), attachBtn, attachedFileNames, audioChunks, filePreviewArea, history, input, messages (+3 more)

### Community 43 - "Community 43"
Cohesion: 0.38
Nodes (7): clearTextureAtlas(), _createAccessibilityTreeNode(), _fullRefresh(), _handleResize(), _refreshRowDimensions(), _refreshRowsDimensions(), setRenderer()

### Community 44 - "Community 44"
Cohesion: 0.53
Nodes (5): save_audio_temp(), transcribe_audio(), AppHandle, Result, String

### Community 45 - "Community 45"
Cohesion: 0.40
Nodes (4): description, identifier, permissions, windows

### Community 47 - "Community 47"
Cohesion: 0.70
Nodes (4): extract_and_save(), get_context(), strip_wikilinks(), String

### Community 51 - "Community 51"
Cohesion: 0.60
Nodes (4): b(), C(), R(), w()

### Community 52 - "Community 52"
Cohesion: 0.17
Nodes (18): _askForLink(), _batchedMemoryCleanup(), _checkLinkProviderResult(), get(), getBufferElements(), getColor(), getCss(), getLine() (+10 more)

### Community 53 - "Community 53"
Cohesion: 0.50
Nodes (4): areSelectionValuesReversed(), finalSelectionEnd(), finalSelectionStart(), _selectToWordAt()

### Community 54 - "Community 54"
Cohesion: 0.50
Nodes (4): Archive, Recent, main, main

### Community 55 - "Community 55"
Cohesion: 0.50
Nodes (3): duckduckgo_search(), Result, String

### Community 57 - "Community 57"
Cohesion: 0.60
Nodes (4): get_radar_url(), refresh_radar(), Result, String

### Community 58 - "Community 58"
Cohesion: 0.50
Nodes (4): addDecoration(), _addLineToZone(), _lineAdjacentToZone(), _lineIntersectsZone()

### Community 70 - "Community 70"
Cohesion: 0.50
Nodes (3): 13:54 | main, 14:03-14:24 | main, 15:35 | main

### Community 71 - "Community 71"
Cohesion: 0.67
Nodes (3): _applyScrollModifier(), getLinesScrolled(), _getPixelsScrolled()

### Community 72 - "Community 72"
Cohesion: 0.67
Nodes (3): _handleLinkHover(), _handleLinkLeave(), _setCellUnderline()

### Community 73 - "Community 73"
Cohesion: 0.67
Nodes (3): _reflow(), _reflowLarger(), _reflowLargerAdjustViewport()

## Knowledge Gaps
- **112 isolated node(s):** `allow`, `session`, `line`, `seed`, `steps` (+107 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **14 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `E` connect `Community 5` to `Community 32`, `Community 1`, `Community 0`, `Community 33`, `Community 36`, `Community 2`, `Community 8`, `Community 9`, `Community 12`, `Community 15`, `Community 16`, `Community 18`, `Community 51`, `Community 52`, `Community 22`, `Community 23`?**
  _High betweenness centrality (0.068) - this node is a cross-community bridge._
- **Why does `d` connect `Community 11` to `Community 32`, `Community 1`, `Community 35`, `Community 8`, `Community 43`, `Community 12`, `Community 15`, `Community 16`, `Community 18`, `Community 21`, `Community 22`?**
  _High betweenness centrality (0.035) - this node is a cross-community bridge._
- **Why does `P` connect `Community 12` to `Community 32`, `Community 1`, `Community 35`, `Community 36`, `Community 8`, `Community 43`, `Community 11`, `Community 15`, `Community 16`, `Community 18`, `Community 21`, `Community 22`?**
  _High betweenness centrality (0.031) - this node is a cross-community bridge._
- **Are the 7 inferred relationships involving `E` (e.g. with `R()` and `w()`) actually correct?**
  _`E` has 7 INFERRED edges - model-reasoned connections that need verification._
- **What connects `allow`, `session`, `line` to the rest of the system?**
  _116 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Community 1` be split into smaller, more focused modules?**
  _Cohesion score 0.043740573152337855 - nodes in this community are weakly interconnected._
- **Should `Community 2` be split into smaller, more focused modules?**
  _Cohesion score 0.07804878048780488 - nodes in this community are weakly interconnected._