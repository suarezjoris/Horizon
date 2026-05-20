from pathlib import Path

VAULT_PATH = Path("~/Documents/Claude RAG").expanduser()
CHROMA_PATH = Path("~/.personal-ai/chroma").expanduser()
MODEL = "dolphin-mixtral:8x7b"
VISION_MODEL = "moondream"
EMBED_MODEL = "nomic-embed-text"
OLLAMA_HOST = "http://localhost:11434"
CHUNK_SIZE = 512
CHUNK_OVERLAP = 64
TOP_K = 6
SIMILARITY_THRESHOLD = 0.4
