from pptx import Presentation
import os

path = os.path.expanduser("~/Documents/Claude RAG/assets/template.pptx")
if not os.path.exists(path):
    print("Template not found")
    exit(1)

prs = Presentation(path)
print(f"--- Layouts in {os.path.basename(path)} ---")
for i, layout in enumerate(prs.slide_layouts):
    print(f"Layout {i}: {layout.name}")
    for shape in layout.placeholders:
        print(f"  - Placeholder {shape.placeholder_format.idx}: {shape.name} ({shape.placeholder_format.type})")
