# DeskWork Capabilities

DeskWork is an advanced desktop agent capable of interacting with your file system, applications, and web browser. Below is a comprehensive list of what you can ask it to do.

## 📂 File System Management
You have full control over your local files.
- **Create Files**: "Create a markdown file called notes.md with a header."
- **Read Files**: "Read src/App.tsx and explain how it works."
- **List Directories**: "Show me what's in the src folder."
- **Search Files**: "Find every file that contains the word 'TODO'."
- **Find Files**: "Where is the logo.png file located?"
- **Update Files**: "Add a new function to the utils.ts file."

## 🖥️ Screen & Visual Automation
DeskWork can "see" your screen to interact with non-API applications.
- **Take Screenshots**: "Take a screenshot of what's on my screen right now."
- **Visual Browsing**: "Open Google, search for Rust tutorials, and tell me what the first result is." (The agent will open, wait, look, and analyze).
- **Find Elements**: "Where is the 'Submit' button on this page?"

## ⌨️ Input Simulation (Macros)
Automate repetitive tasks by simulating human input.
- **Type Text**: "Type 'Hello World' into the active window."
- **Press Keys**: "Press Enter", "Press Tab", "Press Ctrl+C".
- **Mouse Control**: "Move the mouse to 500, 500 and click."

## 🌐 Web & Research
- **Smart Search**: "Search the web for the latest Tauri v2 docs." (Opens browser).
- **Fetch Content**: "Read the content of https://example.com." (Fetches raw HTML/Text for analysis).

## 📄 Office & Content Creation
- **Word Documents**: "Create a DOCX file with a summary of our chat."
- **Presentations**: "Make a slide deck (HTML) about the history of computing."

## 📊 System Information
- **Health Check**: "What is my current CPU and RAM usage?"
- **System Specs**: "What OS version am I running?"

## 🛡️ Safety Features
- **Approval Workflow**: Sensitive actions (like deleting files or executing shell commands) require your explicit approval.
- **Read-Only Mode**: Can be enabled in Settings to prevent any file modifications.

