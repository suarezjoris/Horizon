# transcribe.py
import sys
import os
from faster_whisper import WhisperModel

def transcribe(audio_path):
    model_size = "base" # can be base, small, medium, large-v3
    # Run on GPU if available, else CPU
    model = WhisperModel(model_size, device="auto", compute_type="default")
    
    segments, info = model.transcribe(audio_path, beam_size=5)
    
    text = ""
    for segment in segments:
        text += segment.text + " "
    
    return text.strip()

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python transcribe.py <audio_path>")
        sys.exit(1)
    
    audio_file = sys.argv[1]
    if not os.path.exists(audio_file):
        print(f"File not found: {audio_file}")
        sys.exit(1)
        
    try:
        result = transcribe(audio_file)
        print(result)
    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)
