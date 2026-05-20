# agent/vision.py
import ollama
from config import VISION_MODEL, OLLAMA_HOST

IMAGE_EXTENSIONS = {".png", ".jpg", ".jpeg", ".webp", ".gif", ".bmp"}

def is_image(path: str) -> bool:
    from pathlib import Path
    return Path(path).suffix.lower() in IMAGE_EXTENSIONS

def describe_image(path: str, prompt: str = "Describe this image in detail. Be specific about text, code, UI elements, and errors you see.") -> str:
    import base64
    from config import MODEL
    with open(path, "rb") as f:
        img_b64 = base64.b64encode(f.read()).decode()
    client = ollama.Client(host=OLLAMA_HOST)
    # Free VRAM: unload dolphin before loading moondream
    try:
        client.generate(model=MODEL, prompt="", keep_alive=0)
    except Exception:
        pass
    response = client.chat(
        model=VISION_MODEL,
        messages=[{"role": "user", "content": prompt, "images": [img_b64]}],
        keep_alive=0,  # unload moondream immediately after, so dolphin can reload
    )
    content = response.message.content if hasattr(response, "message") else response["message"]["content"]
    return content or "[image reçue mais description vide]"
