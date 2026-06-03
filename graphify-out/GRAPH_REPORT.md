# Graph Report - Horizon  (2026-06-03)

## Corpus Check
- 55 files · ~205,293 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1759 nodes · 4119 edges · 98 communities (73 shown, 25 thin omitted)
- Extraction: 96% EXTRACTED · 4% INFERRED · 0% AMBIGUOUS · INFERRED: 171 edges (avg confidence: 0.79)
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
- [[_COMMUNITY_Community 82|Community 82]]
- [[_COMMUNITY_Community 83|Community 83]]
- [[_COMMUNITY_Community 84|Community 84]]
- [[_COMMUNITY_Community 85|Community 85]]
- [[_COMMUNITY_Community 86|Community 86]]
- [[_COMMUNITY_Community 87|Community 87]]
- [[_COMMUNITY_Community 88|Community 88]]
- [[_COMMUNITY_Community 89|Community 89]]
- [[_COMMUNITY_Community 90|Community 90]]
- [[_COMMUNITY_Community 91|Community 91]]
- [[_COMMUNITY_Community 92|Community 92]]
- [[_COMMUNITY_Community 93|Community 93]]
- [[_COMMUNITY_Community 94|Community 94]]
- [[_COMMUNITY_Community 95|Community 95]]
- [[_COMMUNITY_Community 96|Community 96]]
- [[_COMMUNITY_Community 97|Community 97]]

## God Nodes (most connected - your core abstractions)
1. `_()` - 476 edges
2. `E` - 120 edges
3. `d` - 97 edges
4. `constructor()` - 79 edges
5. `P` - 68 edges
6. `fire()` - 67 edges
7. `get()` - 55 edges
8. `s()` - 50 edges
9. `i()` - 49 edges
10. `c()` - 49 edges

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

## Communities (98 total, 25 thin omitted)

### Community 0 - "Community 0"
Cohesion: 0.11
Nodes (25): addLineToLink(), addMarker(), _askForLink(), _batchedMemoryCleanup(), _checkLinkProviderResult(), createInstance(), get(), getBlankLine() (+17 more)

### Community 1 - "Community 1"
Cohesion: 0.03
Nodes (75): addDecoration(), _addLineToZone(), _addMouseDownListeners(), addRefreshCallback(), _announceCharacters(), _applyScrollModifier(), _areCoordsInSelection(), areSelectionValuesReversed() (+67 more)

### Community 2 - "Community 2"
Cohesion: 0.07
Nodes (11): S(), _addStyle(), _applyMinimumContrast(), createRow(), f(), forEachByKey(), _getContrastCache(), i() (+3 more)

### Community 3 - "Community 3"
Cohesion: 0.10
Nodes (18): ComposeResult, int, main(), app, security, windows, withGlobalTauri, enable (+10 more)

### Community 4 - "Community 4"
Cohesion: 0.08
Nodes (41): 3, class_type, inputs, 4, class_type, inputs, 5, class_type (+33 more)

### Community 5 - "Community 5"
Cohesion: 0.07
Nodes (5): E, nextStop(), prevStop(), setgCharset(), setgLevel()

### Community 6 - "Community 6"
Cohesion: 0.08
Nodes (41): 3, class_type, inputs, 4, class_type, inputs, 5, class_type (+33 more)

### Community 7 - "Community 7"
Cohesion: 0.15
Nodes (24): Path, read_file_content(), Result, String, PathBuf, Result, String, Vec (+16 more)

### Community 8 - "Community 8"
Cohesion: 0.11
Nodes (9): attachToDom(), enable(), event(), hasRenderer(), register(), _registerBufferChangeListeners(), _registerDecorationListeners(), _registerDimensionChangeListeners() (+1 more)

### Community 9 - "Community 9"
Cohesion: 0.07
Nodes (16): addDcsHandler(), addEncoding(), addEscHandler(), addOscHandler(), addProtocol(), c(), clearHandler(), clearRange() (+8 more)

### Community 10 - "Community 10"
Cohesion: 0.14
Nodes (19): Arc, Mutex, chat(), search_vault(), OpenClaudeState, send_openclaude_raw(), start_openclaude(), AppHandle (+11 more)

### Community 12 - "Community 12"
Cohesion: 0.15
Nodes (5): compositionend(), _finalizeComposition(), _handleAnyTextareaChanges(), keydown(), triggerDataEvent()

### Community 13 - "Community 13"
Cohesion: 0.19
Nodes (17): gallery, generate(), generateBtn, getAssetUrl(), getTauri(), loadGallery(), promptInput, resultImg (+9 more)

### Community 15 - "Community 15"
Cohesion: 0.10
Nodes (3): addCsiHandler(), registerCsiHandler(), s()

### Community 16 - "Community 16"
Cohesion: 0.20
Nodes (7): end(), hook(), put(), reset(), _start(), unhook(), values()

### Community 18 - "Community 18"
Cohesion: 0.13
Nodes (18): _cancelCallback(), clear(), flush(), handleCharSizeChanged(), handleDevicePixelRatioChange(), _handleIntersectionChange(), _handleOptionsChanged(), _injectCss() (+10 more)

### Community 19 - "Community 19"
Cohesion: 0.26
Nodes (16): ChunkMessage, chat_once(), chat_stream(), ChatChunk, ChatRequest, ChunkMessage, describe_image(), embed() (+8 more)

### Community 20 - "Community 20"
Cohesion: 0.25
Nodes (12): Default, config_path(), get_settings(), load(), persist(), save_settings(), Settings, settings_roundtrip_via_json() (+4 more)

### Community 21 - "Community 21"
Cohesion: 0.19
Nodes (17): clearSelection(), disable(), _dragScroll(), _fireEventIfSelectionChanged(), getCoords(), _getMouseEventScrollAmount(), _handleMouseMove(), _handleMouseUp() (+9 more)

### Community 22 - "Community 22"
Cohesion: 0.10
Nodes (8): length(), n(), _reflow(), _reflowLarger(), _reflowLargerAdjustViewport(), _reflowSmaller(), _refreshRowElements(), scroll()

### Community 24 - "Community 24"
Cohesion: 0.05
Nodes (16): cu(), draw(), eu(), fu(), ja(), jm(), ku(), lu() (+8 more)

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
Cohesion: 0.07
Nodes (58): A(), ae(), aw(), b(), ba(), bt(), copy(), d() (+50 more)

### Community 31 - "Community 31"
Cohesion: 0.15
Nodes (15): clearAllMarkers(), clearMarkers(), delete(), deregister(), dispose(), getKeyIterator(), _handleBufferActivate(), insert() (+7 more)

### Community 32 - "Community 32"
Cohesion: 0.07
Nodes (19): activeProtocol(), activeVersion(), b(), debug(), _equalEvents(), fillViewportRows(), fire(), _fireOnCanvasResize() (+11 more)

### Community 33 - "Community 33"
Cohesion: 0.11
Nodes (9): _convertViewportColToCharacterIndex(), decode(), getCell(), getJoinedCharacters(), _getWordAt(), h(), _isCharWordSeparator(), _stringRangesToCellRanges() (+1 more)

### Community 34 - "Community 34"
Cohesion: 0.33
Nodes (11): chunk_text(), cosine_similarity(), Entry, load_index(), reindex(), save_index(), search(), Arc (+3 more)

### Community 35 - "Community 35"
Cohesion: 0.15
Nodes (5): _clearSmoothScrollState(), handleWheel(), scrollLines(), _smoothScroll(), _smoothScrollPercent()

### Community 36 - "Community 36"
Cohesion: 0.08
Nodes (47): bd(), bl(), bs(), cf(), ch(), cl(), Df(), dl() (+39 more)

### Community 37 - "Community 37"
Cohesion: 0.06
Nodes (40): Ag(), ap(), at(), bp(), ct(), ei(), et(), gg() (+32 more)

### Community 38 - "Community 38"
Cohesion: 0.50
Nodes (7): delete_image(), GalleryImage, list_gallery(), save_generated_image(), Result, String, Vec

### Community 40 - "Community 40"
Cohesion: 0.20
Nodes (9): Code Tab User Guide, Example Tasks, Keyboard Shortcuts, Opening a Project, Running Code, Tips, Using the AI Agent, Using the Code Editor (+1 more)

### Community 41 - "Community 41"
Cohesion: 0.19
Nodes (4): clearListeners(), r(), _requestCallback(), warn()

### Community 42 - "Community 42"
Cohesion: 0.18
Nodes (11): addBubble(), attachBtn, attachedFileNames, audioChunks, filePreviewArea, history, input, messages (+3 more)

### Community 43 - "Community 43"
Cohesion: 0.09
Nodes (7): buffer(), clearTextureAtlas(), _fullRefresh(), hasSelection(), P, registerLinkProvider(), setRenderer()

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
Cohesion: 0.18
Nodes (5): b(), C(), R(), w(), L()

### Community 52 - "Community 52"
Cohesion: 0.11
Nodes (24): ad(), al(), Bf(), C(), cd(), dd(), ed, fd() (+16 more)

### Community 53 - "Community 53"
Cohesion: 0.07
Nodes (31): Ai(), ar(), Ce(), Ci(), cr(), displayable(), dr(), Er() (+23 more)

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
Cohesion: 0.15
Nodes (15): Eg(), hf(), Ig(), J(), Jc(), lf(), Lg(), Ng() (+7 more)

### Community 70 - "Community 70"
Cohesion: 0.50
Nodes (3): 13:54 | main, 14:03-14:24 | main, 15:35 | main

### Community 71 - "Community 71"
Cohesion: 0.27
Nodes (11): ax(), Bm(), F(), km(), lm(), Nm(), Rm(), Vm() (+3 more)

### Community 72 - "Community 72"
Cohesion: 0.20
Nodes (11): Be(), clamp(), formatHsl(), ge(), hn(), jn(), pn(), toString() (+3 more)

### Community 73 - "Community 73"
Cohesion: 0.24
Nodes (4): compositionstart(), _handleBoundaryFocus(), handleFocus(), shouldColumnSelect()

### Community 82 - "Community 82"
Cohesion: 0.22
Nodes (10): ao(), co(), fo(), Hi(), ji(), uo(), Vi(), Wi() (+2 more)

### Community 83 - "Community 83"
Cohesion: 0.31
Nodes (9): cp(), dp(), fp(), hp(), lp(), pp(), sp(), up() (+1 more)

### Community 84 - "Community 84"
Cohesion: 0.40
Nodes (5): an(), en(), mn(), nn(), sn()

### Community 85 - "Community 85"
Cohesion: 0.40
Nodes (5): bezierCurveTo(), Bx(), ow(), rx(), Vx()

### Community 86 - "Community 86"
Cohesion: 0.40
Nodes (5): Dh(), Ih(), Ph(), qh(), uh()

### Community 87 - "Community 87"
Cohesion: 0.67
Nodes (3): go(), tn(), wn()

### Community 88 - "Community 88"
Cohesion: 0.67
Nodes (3): my(), Ry(), vg()

## Knowledge Gaps
- **112 isolated node(s):** `allow`, `session`, `line`, `seed`, `steps` (+107 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **25 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `_()` connect `Community 17` to `Community 12`, `Community 24`, `Community 30`, `Community 36`, `Community 37`, `Community 52`, `Community 53`, `Community 58`, `Community 71`, `Community 72`, `Community 82`, `Community 83`, `Community 84`, `Community 85`, `Community 86`, `Community 87`, `Community 88`, `Community 89`, `Community 90`, `Community 91`, `Community 92`, `Community 93`, `Community 94`, `Community 95`, `Community 96`, `Community 97`?**
  _High betweenness centrality (0.311) - this node is a cross-community bridge._
- **Why does `L()` connect `Community 51` to `Community 1`, `Community 36`, `Community 37`, `Community 9`, `Community 52`, `Community 30`, `Community 31`?**
  _High betweenness centrality (0.145) - this node is a cross-community bridge._
- **Why does `E` connect `Community 5` to `Community 0`, `Community 1`, `Community 32`, `Community 33`, `Community 9`, `Community 41`, `Community 12`, `Community 15`, `Community 16`, `Community 51`, `Community 22`, `Community 23`?**
  _High betweenness centrality (0.115) - this node is a cross-community bridge._
- **Are the 7 inferred relationships involving `E` (e.g. with `R()` and `w()`) actually correct?**
  _`E` has 7 INFERRED edges - model-reasoned connections that need verification._
- **What connects `allow`, `session`, `line` to the rest of the system?**
  _116 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Community 0` be split into smaller, more focused modules?**
  _Cohesion score 0.11394557823129252 - nodes in this community are weakly interconnected._
- **Should `Community 1` be split into smaller, more focused modules?**
  _Cohesion score 0.028882093102276588 - nodes in this community are weakly interconnected._