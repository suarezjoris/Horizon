import sys
import os
import docx
from pptx import Presentation
import pandas as pd

def extract_text(path):
    ext = os.path.splitext(path)[1].lower()
    
    if ext == ".docx":
        doc = docx.Document(path)
        return "\n".join([para.text for para in doc.paragraphs])
    
    elif ext == ".pptx":
        prs = Presentation(path)
        text_runs = []
        for slide in prs.slides:
            for shape in slide.shapes:
                if hasattr(shape, "text"):
                    text_runs.append(shape.text)
        return "\n".join(text_runs)
    
    elif ext == ".xlsx" or ext == ".xls":
        df = pd.read_excel(path)
        return df.to_string()
    
    return ""

if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(1)
    
    file_path = sys.argv[1]
    if not os.path.exists(file_path):
        print(f"File not found: {file_path}", file=sys.stderr)
        sys.exit(1)
        
    try:
        print(extract_text(file_path))
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)
