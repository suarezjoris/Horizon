# Code Tab User Guide

## Opening a Project

1. Click the **⌨️ Code** tab in Horizon
2. Enter project path in the file browser input (e.g., `/home/joris/Projects/my-project`)
3. Press Enter to load project files

## Using the Code Editor

- **Left sidebar**: File browser - click any file to open
- **Toggle button (☰)**: Collapse/expand file browser
- **Main editor**: Shows file content with syntax highlighting (Python, Rust, JavaScript, JSON, HTML, CSS, etc.)
- **Breadcrumb**: Shows currently open file path

## Running Code

**Terminal** (bottom panel):
- Type shell commands directly
- Press Enter to execute
- View output in real-time

## Using the AI Agent

**Chat bubble** (bottom-right):
1. Type a code task: "Write a function that...", "Fix the error on line 45", "Add error handling", etc.
2. Press Ctrl+Enter or click Send
3. Watch the agent:
   - Read and analyze your code
   - Write or modify files
   - Run tests automatically
   - Fix errors and retry until code works
   - Narrate each step in real-time

## Example Tasks

- "Write a function that sums an array"
- "Add error handling to the login function"
- "Run tests and fix any failures"
- "Fix the TypeError on line 32"

## What the Agent Can Do

✓ Read and understand your project structure
✓ Write, modify, and delete code files
✓ Run tests, linters, and compilers
✓ Analyze error messages and suggest fixes
✓ Retry automatically until tests pass
✓ Work with Rust, JavaScript, Python projects

## Keyboard Shortcuts

- **Ctrl+Enter** in chat: Send message
- **Shift+Enter** in chat: New line
- **Enter** in terminal: Execute command

## Tips

- More specific tasks work better: "Add function `validate_email()` that checks email format using regex"
- The agent works locally - no data leaves your machine
- If stuck, provide more context: "In the login.py file, add error handling..."

---

Generated with Horizon v2 Coding IDE
