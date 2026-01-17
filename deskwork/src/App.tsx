import { useState, useEffect, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { Send, Terminal, Settings as SettingsIcon, MessageSquare, Loader2, CheckCircle2, FileText, FolderOpen, Plus, Folder, LayoutTemplate, Globe, Cpu, Search, Monitor, Star, StarOff, Edit3, AlertOctagon, Copy } from "lucide-react";
import { SettingsModal } from "./SettingsModal";
import { RightSidebar } from "./RightSidebar";
import "./App.css";

interface Message {
  role: "user" | "assistant";
  content: string;
}

interface ActivityEvent {
  id: string;
  status: "pending" | "running" | "success" | "error";
  message: string;
  timestamp: number;
}

interface Session {
  id: string;
  title: string;
  updated_at: number;
  messages?: any[];
  pinned?: boolean;
}

interface PendingApproval {
  id: string;
  action: string;
  reason: string;
  expires_at: number;
}

function App() {
  const [input, setInput] = useState("");
  const [messages, setMessages] = useState<Message[]>([]);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [currentSessionId, setCurrentSessionId] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [currentActivity, setCurrentActivity] = useState<ActivityEvent | null>(null);
  const [workingDir, setWorkingDir] = useState<string | null>(null);
  const [pendingApprovals, setPendingApprovals] = useState<PendingApproval[]>([]);
  const [sessionSearch, setSessionSearch] = useState("");
  const [displayPrefs, setDisplayPrefs] = useState<{ reduced_motion: boolean; high_contrast: boolean }>({ reduced_motion: false, high_contrast: false });
  const [errorToast, setErrorToast] = useState<{ message: string; detail?: string } | null>(null);

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const streamingMessageIndex = useRef<number | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  };

  useEffect(() => {
    scrollToBottom();
  }, [messages, currentActivity, isLoading]);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const activeApprovals = useMemo(
    () => pendingApprovals.filter((p) => !p.expires_at || p.expires_at * 1000 > Date.now()),
    [pendingApprovals]
  );

  const filteredSessions = useMemo(() => {
    const q = sessionSearch.toLowerCase();
    const list = q ? sessions.filter((s) => s.title.toLowerCase().includes(q) || s.id.toLowerCase().includes(q)) : sessions;
    return [...list].sort((a, b) => {
      const ap = a.pinned ? 1 : 0;
      const bp = b.pinned ? 1 : 0;
      if (ap !== bp) return bp - ap;
      return b.updated_at - a.updated_at;
    });
  }, [sessions, sessionSearch]);

  // Load display preferences (reduced motion / high contrast)
  useEffect(() => {
    invoke<any>("get_settings")
      .then((settings) => {
        setDisplayPrefs({
          reduced_motion: Boolean(settings.reduced_motion),
          high_contrast: Boolean(settings.high_contrast),
        });
      })
      .catch((e) => console.error("Failed to load settings", e));
  }, []);

  useEffect(() => {
    const root = document.documentElement;
    root.classList.toggle("reduced-motion", displayPrefs.reduced_motion);
    root.classList.toggle("high-contrast", displayPrefs.high_contrast);
  }, [displayPrefs]);


  const normalizeMessages = (rawMessages: any[] = []): Message[] => {
    return rawMessages
      .filter((m) => m.role === "user" || m.role === "assistant")
      .map((m) => {
        let text = "";
        if (typeof m.content === "string") {
          text = m.content;
        } else if (m.content?.Text) {
          text = m.content.Text;
        } else if (m.content?.text) {
          text = m.content.text;
        } else if (m.content?.Parts) {
          text = m.content.Parts.map((p: any) => p.text).filter(Boolean).join("\n");
        } else if (Array.isArray(m.content)) {
          text = m.content.map((p: any) => p.text).filter(Boolean).join("\n");
        }
        return {
          role: m.role,
          content: text || "[Unsupported message format omitted]",
        } as Message;
      });
  };

  useEffect(() => {
    loadSessions();
    const unlisten = listen<ActivityEvent>("activity", (event) => {
      if (event.payload.status === "success" || event.payload.status === "error") {
         setCurrentActivity(event.payload);
         setTimeout(() => setCurrentActivity(null), 2000); 
      } else {
        setCurrentActivity(event.payload);
      }
    });

    const unlistenApprovalReq = listen<any>("approval_request", (event) => {
      const payload = event.payload as any;
      setPendingApprovals((prev) => {
        const filtered = prev.filter((p) => p.id !== payload.id);
        return [...filtered, {
          id: payload.id,
          action: payload.action,
          reason: payload.reason,
          expires_at: payload.expires_at ?? 0,
        }];
      });
    });

    const unlistenApprovalResolved = listen<any>("approval_resolved", (event) => {
      const payload = event.payload as any;
      setPendingApprovals((prev) => prev.filter((p) => p.id !== payload.id));
    });

    const unlistenStream = listen<any>("chat_stream", (event) => {
      const payload = event.payload as any;
      if (payload.done) {
        streamingMessageIndex.current = null;
        return;
      }
      setMessages((prev) => {
        if (streamingMessageIndex.current === null) {
          const idx = prev.length;
          streamingMessageIndex.current = idx;
          return [...prev, { role: "assistant", content: payload.token || "" }];
        }
        return prev.map((m, idx) =>
          idx === streamingMessageIndex.current
            ? { ...m, content: `${m.content || ""}${payload.token || ""}` }
            : m,
        );
      });
    });

    return () => {
      unlisten.then((f) => f());
      unlistenApprovalReq.then((f) => f());
      unlistenApprovalResolved.then((f) => f());
      unlistenStream.then((f) => f());
    };
  }, []);

  // Keyboard shortcuts: Ctrl+Enter to send, Ctrl+Shift+A to approve first pending
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
        e.preventDefault();
        handleSubmit();
      }
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "a") {
        if (activeApprovals.length > 0) {
          e.preventDefault();
          handleSubmit(undefined, `approve ${activeApprovals[0].id}`);
        }
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [activeApprovals, handleSubmit]);

  async function loadSessions() {
    try {
      const list = await invoke<Session[]>("list_sessions");
      setSessions(list);
    } catch (e) {
      console.error("Failed to load sessions", e);
    }
  }

  async function handleNewSession() {
    try {
      const session = await invoke<Session>("create_session", { title: "New Chat" });
      setSessions(prev => [session, ...prev]);
      setCurrentSessionId(session.id);
      setMessages([]);
    } catch (e) {
      console.error("Failed create session", e);
    }
  }

  async function handleSwitchSession(id: string) {
    try {
      const session: any = await invoke("switch_session", { session_id: id });
      setCurrentSessionId(session.id);
      setMessages(normalizeMessages(session.messages));
    } catch (e) {
      console.error("Failed switch session", e);
    }
  }

  async function handleRenameSession(id: string) {
    const title = prompt("New title");
    if (!title) return;
    try {
      const updated = await invoke<Session>("rename_session", { session_id: id, title });
      setSessions((prev) => prev.map((s) => (s.id === updated.id ? { ...s, title: updated.title } : s)));
    } catch (e) {
      console.error("Failed rename session", e);
    }
  }

  async function handlePinSession(id: string, pinned: boolean) {
    try {
      const updated = await invoke<Session>("toggle_pin", { session_id: id, pinned });
      setSessions((prev) => prev.map((s) => (s.id === updated.id ? { ...s, pinned: updated.pinned } : s)));
      loadSessions();
    } catch (e) {
      console.error("Failed pin session", e);
    }
  }

  async function handleExportSessions() {
    try {
      const payload = await invoke<string>("export_sessions");
      await navigator.clipboard.writeText(payload);
      alert("Sessions exported to clipboard (tool outputs redacted).");
    } catch (e) {
      console.error("Failed export sessions", e);
      alert("Failed to export sessions");
    }
  }

  async function handleImportSessions() {
    const payload = prompt("Paste exported sessions JSON");
    if (!payload) return;
    try {
      const count = await invoke<number>("import_sessions", { payload });
      alert(`Imported ${count} sessions`);
      loadSessions();
    } catch (e) {
      console.error("Failed import sessions", e);
      alert("Failed to import sessions");
    }
  }

  async function handleFolderSelect() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
      });
      if (selected) {
        setWorkingDir(selected as string);
      }
    } catch (e) {
      console.error("Failed to open dialog", e);
    }
  }

  async function handleSubmit(e?: React.FormEvent, promptOverride?: string) {
    if (e) e.preventDefault();
    if (!workingDir) return;

    const promptToSend = promptOverride || input;
    if (!promptToSend.trim() || isLoading) return;

    // Create session if none exists
    if (!currentSessionId) {
       await handleNewSession();
    }

    const userMsg: Message = { role: "user", content: promptToSend };
    setMessages((prev) => [...prev, userMsg]);
    streamingMessageIndex.current = null;
    setInput("");
    setIsLoading(true);
    setCurrentActivity(null);
    inputRef.current?.focus();

    try {
      const response = await invoke<string>("chat", { 
        prompt: promptToSend,
        workingDir: workingDir,
        session_id: currentSessionId
      });
      setMessages((prev) => {
        if (streamingMessageIndex.current !== null) {
          return prev.map((m, idx) => idx === streamingMessageIndex.current ? { ...m, content: response } : m);
        }
        return [...prev, { role: "assistant", content: response }];
      });
      loadSessions(); // Refresh list to update titles/timestamps if needed
    } catch (error) {
      console.error("Error calling backend:", error);
      setErrorToast({ message: "Chat failed", detail: String(error) });
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: `Error: ${error}` },
      ]);
    } finally {
      setIsLoading(false);
      setCurrentActivity(null);
    }
  }

  // ... (Keep existing helpers: getActivityIcon, QuickAction) ...
  const getActivityIcon = (status: string, message: string) => {
    if (status === "running" || status === "pending") return <Loader2 className="w-4 h-4 animate-spin text-indigo-400" />;
    if (status === "error") return <CheckCircle2 className="w-4 h-4 text-red-400" />;
    if (message.includes("File")) return <FileText className="w-4 h-4 text-emerald-400" />;
    if (message.includes("Listing")) return <FolderOpen className="w-4 h-4 text-emerald-400" />;
    return <CheckCircle2 className="w-4 h-4 text-emerald-400" />;
  };

  const QuickAction = ({ icon: Icon, title, prompt }: { icon: any, title: string, prompt: string }) => (
    <button 
      onClick={() => handleSubmit(undefined, prompt)}
      disabled={!workingDir}
      className={`flex items-center gap-3 p-4 border rounded-xl transition-all group text-left ${
        workingDir 
          ? "bg-zinc-900/50 border-white/5 hover:border-indigo-500/30 hover:bg-zinc-900 cursor-pointer" 
          : "bg-zinc-900/20 border-white/5 opacity-50 cursor-not-allowed"
      }`}
    >
      <div className={`w-10 h-10 rounded-lg flex items-center justify-center transition-colors ${
        workingDir ? "bg-zinc-800 group-hover:bg-indigo-500/10" : "bg-zinc-800/50"
      }`}>
        <Icon className={`w-5 h-5 ${workingDir ? "text-zinc-400 group-hover:text-indigo-400" : "text-zinc-600"}`} />
      </div>
      <span className={`text-sm font-medium ${workingDir ? "text-zinc-300 group-hover:text-white" : "text-zinc-600"}`}>
        {title}
      </span>
    </button>
  );

  return (
    <div className="flex h-screen bg-zinc-950 text-zinc-100 font-sans overflow-hidden selection:bg-indigo-500/30">
      <SettingsModal 
        isOpen={isSettingsOpen} 
        onClose={() => setIsSettingsOpen(false)} 
      />

      {/* Sidebar (Navigation) */}
      <div className="w-[72px] lg:w-64 bg-zinc-900/50 border-r border-white/5 flex flex-col backdrop-blur-xl transition-all duration-300">
        <div className="h-16 flex items-center justify-center lg:justify-start lg:px-6 border-b border-white/5">
          <div className="w-8 h-8 rounded-xl bg-gradient-to-br from-indigo-500 to-blue-600 flex items-center justify-center shadow-lg shadow-indigo-500/20">
            <Terminal className="w-4 h-4 text-white" />
          </div>
          <span className="hidden lg:block ml-3 font-semibold tracking-tight">DeskWork</span>
        </div>
        
        <div className="flex-1 py-4 space-y-2 px-3 overflow-y-auto">
          <button 
            onClick={handleNewSession}
            className="w-full flex items-center justify-center lg:justify-start lg:px-3 py-2.5 bg-indigo-600/10 text-indigo-400 hover:bg-indigo-600/20 rounded-xl transition-all group border border-indigo-500/20 mb-4"
          >
            <Plus className="w-5 h-5" />
            <span className="hidden lg:block ml-3 text-sm font-medium">New Thread</span>
          </button>

          <div className="hidden lg:block px-1 pb-2">
            <input
              value={sessionSearch}
              onChange={(e) => setSessionSearch(e.target.value)}
              placeholder="Search sessions..."
              className="w-full px-3 py-2 text-sm rounded-lg bg-zinc-900 border border-white/5 text-white placeholder-zinc-500 focus:border-indigo-500/50 outline-none"
            />
          </div>

          <div className="hidden lg:flex gap-2 px-1 pb-2">
            <button onClick={handleExportSessions} className="text-[11px] px-2 py-1 rounded-md bg-white/5 text-zinc-300 border border-white/5 hover:border-indigo-500/40">Export</button>
            <button onClick={handleImportSessions} className="text-[11px] px-2 py-1 rounded-md bg-white/5 text-zinc-300 border border-white/5 hover:border-indigo-500/40">Import</button>
          </div>

          <div className="hidden lg:block px-3 pb-2 text-[10px] font-semibold text-zinc-500 uppercase tracking-wider">
            History
          </div>
          
          {filteredSessions.map((session) => (
            <div
              key={session.id}
              className={`w-full flex items-center gap-2 lg:px-3 py-2.5 rounded-xl transition-all group ${
                currentSessionId === session.id 
                  ? "bg-zinc-800 text-white" 
                  : "text-zinc-400 hover:text-white hover:bg-white/5"
              }`}
            >
              <button
                onClick={() => handleSwitchSession(session.id)}
                className="flex items-center gap-2 flex-1 text-left"
              >
                <MessageSquare className="w-4 h-4 shrink-0" />
                <span className="hidden lg:block text-sm font-medium truncate">
                  {session.title || "Untitled Chat"}
                </span>
              </button>
              <button
                onClick={() => handleRenameSession(session.id)}
                className="text-zinc-500 hover:text-white"
                title="Rename"
              >
                <Edit3 className="w-4 h-4" />
              </button>
              <button
                onClick={() => handlePinSession(session.id, !session.pinned)}
                className={`${session.pinned ? "text-amber-400" : "text-zinc-500 hover:text-white"}`}
                title="Pin"
              >
                {session.pinned ? <Star className="w-4 h-4" /> : <StarOff className="w-4 h-4" />}
              </button>
            </div>
          ))}
        </div>

        <div className="p-3 border-t border-white/5">
          <button 
            onClick={() => setIsSettingsOpen(true)}
            className="w-full flex items-center justify-center lg:justify-start lg:px-3 py-2.5 text-zinc-400 hover:text-white hover:bg-white/5 rounded-xl transition-all group"
          >
            <SettingsIcon className="w-5 h-5 group-hover:rotate-45 transition-transform duration-500" />
            <span className="hidden lg:block ml-3 text-sm font-medium">Settings</span>
          </button>
        </div>
      </div>

      {/* Main Content */}
      <div className="flex-1 flex flex-col h-full relative bg-zinc-950/50">
        {/* Messages Area */}
        <div className="flex-1 overflow-y-auto p-4 lg:p-8 space-y-8 scroll-smooth">
          {messages.length === 0 && !currentActivity ? (
            <div className="flex flex-col items-center justify-center h-full max-w-4xl mx-auto w-full animate-in fade-in duration-700">
              <div className="mb-12 text-center space-y-2">
                <h2 className="text-3xl font-semibold bg-gradient-to-br from-white to-zinc-500 bg-clip-text text-transparent">
                  Good afternoon
                </h2>
                <p className="text-zinc-500">
                  {workingDir 
                    ? "Ready to collaborate in " + workingDir.split(/[\\/]/).pop()
                    : "Please select a folder to start working."}
                </p>
                
                {!workingDir && (
                  <div className="pt-4">
                    <button
                      onClick={handleFolderSelect}
                      className="inline-flex items-center gap-2 px-6 py-3 bg-indigo-600 hover:bg-indigo-500 text-white rounded-xl font-medium shadow-lg shadow-indigo-500/20 transition-all hover:scale-105"
                    >
                      <Folder className="w-5 h-5" />
                      Select Project Folder
                    </button>
                  </div>
                )}
              </div>

              <div className="grid grid-cols-1 md:grid-cols-3 gap-4 w-full px-8 opacity-90">
                <QuickAction icon={FileText} title="Create a file" prompt="Create a new file called 'notes.md' and add a header." />
                <QuickAction icon={Globe} title="Web Research" prompt="Search the web for the latest Rust Tauri v2 documentation." />
                <QuickAction icon={LayoutTemplate} title="Make a prototype" prompt="Create a basic HTML/CSS prototype structure." />
                <QuickAction icon={Cpu} title="System Health" prompt="Check my current system CPU and Memory usage." />
                <QuickAction icon={Monitor} title="Launch App" prompt="Launch Notepad." />
                <QuickAction icon={Search} title="Search Code" prompt="Search for 'TODO' in this folder." />
              </div>
            </div>
          ) : (
            <>
              {messages.map((msg, idx) => (
                <div
                  key={idx}
                  className={`flex ${msg.role === "user" ? "justify-end" : "justify-start"} animate-in slide-in-from-bottom-2 duration-300`}
                >
                  <div
                    className={`max-w-[85%] lg:max-w-[70%] rounded-2xl px-5 py-3.5 text-sm leading-relaxed shadow-sm ${
                      msg.role === "user"
                        ? "bg-indigo-600 text-white shadow-indigo-500/10 rounded-br-sm"
                        : "bg-zinc-900 border border-white/5 text-zinc-200 rounded-bl-sm"
                    }`}
                  >
                    <div className="whitespace-pre-wrap font-normal">
                      {msg.content}
                    </div>
                  </div>
                </div>
              ))}
            </>
          )}

          {(isLoading || currentActivity) && (
             <div className="flex justify-start animate-in fade-in duration-300">
               <div className="bg-zinc-900/50 border border-white/5 rounded-2xl px-4 py-3 rounded-bl-sm flex gap-3 items-center backdrop-blur-sm max-w-md">
                 <div className="shrink-0 flex items-center justify-center w-5 h-5">
                   {currentActivity ? (
                     getActivityIcon(currentActivity.status, currentActivity.message)
                   ) : (
                     <Loader2 className="w-4 h-4 animate-spin text-zinc-500" />
                   )}
                 </div>
                 <div className="min-w-0">
                    {currentActivity ? (
                      <div className="flex flex-col">
                        <span className="text-sm text-zinc-300 font-medium truncate">
                          {currentActivity.message}
                        </span>
                        <span className="text-[10px] text-zinc-500 uppercase tracking-wider font-semibold">
                          {currentActivity.status === "running" ? "Working..." : "Completed"}
                        </span>
                      </div>
                    ) : (
                      <span className="text-sm text-zinc-400 italic">Thinking...</span>
                    )}
                 </div>
               </div>
             </div>
          )}
          <div ref={messagesEndRef} className="h-4" />
        </div>

        {/* Input Area */}
        <div className="p-4 lg:p-6 pb-6 lg:pb-8">
          {errorToast && (
            <div className="max-w-4xl mx-auto mb-3 px-4 py-3 rounded-xl border border-red-500/40 bg-red-500/10 text-red-100 text-sm flex items-start gap-2">
              <AlertOctagon className="w-4 h-4 mt-0.5 shrink-0" />
              <div className="flex-1">
                <div className="font-semibold">{errorToast.message}</div>
                {errorToast.detail && <div className="text-xs text-red-100/80 break-words">{errorToast.detail}</div>}
              </div>
              <div className="flex gap-2">
                <button
                  type="button"
                  onClick={() => setErrorToast(null)}
                  className="px-2 py-1 rounded-md bg-white/10 border border-white/20 text-xs"
                >
                  Dismiss
                </button>
                {errorToast.detail && (
                  <button
                    type="button"
                    onClick={() => navigator.clipboard.writeText(errorToast.detail || "")}
                    className="px-2 py-1 rounded-md bg-white/10 border border-white/20 text-xs inline-flex items-center gap-1"
                  >
                    <Copy className="w-3 h-3" /> Copy
                  </button>
                )}
              </div>
            </div>
          )}
          {activeApprovals.length > 0 && (
            <div className="max-w-4xl mx-auto mb-3 space-y-2">
              {activeApprovals.map((p) => (
                <div key={p.id} className="px-4 py-3 rounded-xl border border-amber-500/40 bg-amber-500/10 text-amber-100 flex items-start justify-between gap-3">
                  <div>
                    <div className="text-sm font-semibold">Approval needed: {p.action}</div>
                    <div className="text-xs text-amber-200/80">{p.reason}</div>
                    {p.expires_at ? (
                      <div className="text-[11px] text-amber-200/70 mt-1">Expires at {new Date(p.expires_at * 1000).toLocaleTimeString()}</div>
                    ) : null}
                  </div>
                  <div className="flex gap-2">
                    <button
                      type="button"
                      onClick={() => handleSubmit(undefined, `approve ${p.id}`)}
                      className="px-3 py-2 rounded-lg bg-emerald-500/20 border border-emerald-400/40 text-emerald-100 text-xs font-semibold hover:bg-emerald-500/30"
                    >
                      Approve (Ctrl/Cmd+Shift+A)
                    </button>
                    <button
                      type="button"
                      onClick={() => handleSubmit(undefined, `deny ${p.id}`)}
                      className="px-3 py-2 rounded-lg bg-red-500/10 border border-red-400/40 text-red-100 text-xs font-semibold hover:bg-red-500/20"
                    >
                      Deny
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
          <form
            onSubmit={(e) => handleSubmit(e)}
            className="max-w-4xl mx-auto relative group flex flex-col gap-3"
          >
            <div className="flex items-center gap-2">
               <button
                 type="button"
                 onClick={handleFolderSelect}
                 className={`flex items-center gap-2 px-3 py-1.5 rounded-lg border transition-all text-xs font-medium ${
                    workingDir 
                    ? "bg-zinc-900 border-white/10 hover:border-indigo-500/50 text-zinc-400 hover:text-white" 
                    : "bg-indigo-500/10 border-indigo-500/50 text-indigo-400 animate-pulse"
                 }`}
               >
                 <Folder className="w-3.5 h-3.5" />
                 {workingDir ? workingDir.split(/[\\/]/).pop() : "Select Working Folder"}
               </button>
               {workingDir && (
                 <span className="text-[10px] text-zinc-600 truncate max-w-[200px]">
                   {workingDir}
                 </span>
               )}
            </div>

            <div className={`relative flex items-center bg-zinc-900/80 backdrop-blur-xl border rounded-2xl shadow-2xl transition-all ${
                workingDir 
                ? "border-white/10 focus-within:border-indigo-500/50 focus-within:ring-1 focus-within:ring-indigo-500/50" 
                : "border-white/5 opacity-50 cursor-not-allowed"
            }`}>
              <input
                type="text"
                value={input}
                onChange={(e) => setInput(e.target.value)}
                placeholder={workingDir ? "Ask anything..." : "Please select a folder first..."}
                className="w-full bg-transparent text-white placeholder-zinc-500 px-5 py-4 focus:outline-none text-[15px] disabled:cursor-not-allowed"
                disabled={isLoading || !workingDir}
              />
              <button
                type="submit"
                disabled={isLoading || !input.trim() || !workingDir}
                className="mr-2 p-2.5 rounded-xl bg-white/5 hover:bg-indigo-500 text-zinc-400 hover:text-white transition-all disabled:opacity-0 disabled:scale-75"
              >
                <Send className="w-4 h-4" />
              </button>
            </div>
          </form>
          <div className="text-center mt-3">
             <span className="text-[10px] text-zinc-600 font-medium tracking-wide uppercase">
               DeskWork AI • Native & Secure
             </span>
          </div>
        </div>
      </div>

      {/* Right Sidebar */}
      <RightSidebar workingDir={workingDir} />
    </div>
  );
}

export default App;
