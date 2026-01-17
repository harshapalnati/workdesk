import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { X, Save, Key, Cpu } from "lucide-react";

interface AppSettings {
  api_key: string;
  model: string;
  openai_api_key?: string;
  provider?: string;
  read_only?: boolean;
  structured_logs?: boolean;
  reduced_motion?: boolean;
  high_contrast?: boolean;
}

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export function SettingsModal({ isOpen, onClose }: SettingsModalProps) {
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("gpt-4o");
  const [readOnly, setReadOnly] = useState(false);
  const [structuredLogs, setStructuredLogs] = useState(false);
  const [provider, setProvider] = useState("openai");
  const [reducedMotion, setReducedMotion] = useState(false);
  const [highContrast, setHighContrast] = useState(false);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (isOpen) {
      loadSettings();
    }
  }, [isOpen]);

  async function loadSettings() {
    try {
      const settings = await invoke<AppSettings>("get_settings");
      setApiKey(settings.openai_api_key || settings.api_key);
      setModel(settings.model);
      setReadOnly(Boolean(settings.read_only));
      setStructuredLogs(Boolean(settings.structured_logs));
      setProvider(settings.provider || "openai");
      setReducedMotion(Boolean(settings.reduced_motion));
      setHighContrast(Boolean(settings.high_contrast));
    } catch (e) {
      console.error("Failed to load settings", e);
    }
  }

  async function handleSave() {
    setLoading(true);
    try {
      await invoke("save_settings", { settings: { api_key: apiKey, openai_api_key: apiKey, model, provider, read_only: readOnly, structured_logs: structuredLogs, reduced_motion: reducedMotion, high_contrast: highContrast } });
      onClose();
    } catch (e) {
      console.error("Failed to save settings", e);
      alert("Failed to save settings");
    } finally {
      setLoading(false);
    }
  }

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 animate-in fade-in duration-200">
      <div className="bg-zinc-900 border border-white/10 rounded-2xl w-[420px] shadow-2xl p-0 relative animate-in zoom-in-95 duration-200 overflow-hidden">
        {/* Header */}
        <div className="bg-white/5 px-6 py-4 flex items-center justify-between border-b border-white/5">
          <h2 className="text-lg font-semibold text-white">Settings</h2>
          <button 
            onClick={onClose}
            className="text-zinc-400 hover:text-white transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="p-6 space-y-6">
          {/* API Key Section */}
          <div className="space-y-3">
            <label className="flex items-center gap-2 text-xs font-semibold text-zinc-400 uppercase tracking-wider">
              <Key className="w-3.5 h-3.5" />
              OpenAI API Key
            </label>
            <div className="relative group">
              <input
                type="password"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                placeholder="sk-..."
                className="w-full bg-black/40 border border-white/10 text-white rounded-xl px-4 py-3 text-sm focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/50 transition-all placeholder:text-zinc-700"
              />
            </div>
          </div>

          {/* Model Section */}
          <div className="space-y-3">
            <label className="flex items-center gap-2 text-xs font-semibold text-zinc-400 uppercase tracking-wider">
              <Cpu className="w-3.5 h-3.5" />
              Model Selection
            </label>
            <div className="space-y-2">
              <div className="relative">
                <select
                  value={provider}
                  onChange={(e) => setProvider(e.target.value)}
                  className="w-full bg-black/40 border border-white/10 text-white rounded-xl px-4 py-3 text-sm focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/50 transition-all appearance-none cursor-pointer mb-2"
                >
                  <option value="openai">OpenAI</option>
                  {/* Future: add other providers */}
                </select>
                <div className="absolute right-4 top-[18px] pointer-events-none text-zinc-500">
                  <svg width="10" height="6" viewBox="0 0 10 6" fill="none" xmlns="http://www.w3.org/2000/svg">
                    <path d="M1 1L5 5L9 1" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                  </svg>
                </div>
              </div>
            </div>
            <div className="relative">
              <select
                value={model}
                onChange={(e) => setModel(e.target.value)}
                className="w-full bg-black/40 border border-white/10 text-white rounded-xl px-4 py-3 text-sm focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/50 transition-all appearance-none cursor-pointer"
              >
                <option value="gpt-4o">GPT-4o (Recommended)</option>
                <option value="gpt-4-turbo">GPT-4 Turbo</option>
                <option value="gpt-4">GPT-4</option>
                <option value="gpt-3.5-turbo">GPT-3.5 Turbo</option>
              </select>
              <div className="absolute right-4 top-1/2 -translate-y-1/2 pointer-events-none text-zinc-500">
                <svg width="10" height="6" viewBox="0 0 10 6" fill="none" xmlns="http://www.w3.org/2000/svg">
                  <path d="M1 1L5 5L9 1" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                </svg>
              </div>
            </div>
          </div>

          {/* Read-only Toggle */}
          <div className="space-y-2">
            <div className="flex items-center justify-between px-3 py-3 rounded-xl border border-white/10 bg-black/30">
              <div>
                <div className="text-sm font-semibold text-white">Read-only mode</div>
                <div className="text-xs text-zinc-500">Block write/exec tools unless explicitly approved.</div>
              </div>
              <label className="inline-flex items-center cursor-pointer">
                <input
                  type="checkbox"
                  className="sr-only peer"
                  checked={readOnly}
                  onChange={(e) => setReadOnly(e.target.checked)}
                />
                <div className="w-11 h-6 bg-zinc-700 peer-focus:outline-none rounded-full peer peer-checked:bg-indigo-500 relative transition-colors">
                  <div className={`absolute top-1 left-1 w-4 h-4 bg-white rounded-full transition-transform ${readOnly ? "translate-x-5" : ""}`}></div>
                </div>
              </label>
            </div>
          </div>

          {/* Structured logs toggle */}
          <div className="space-y-2">
            <div className="flex items-center justify-between px-3 py-3 rounded-xl border border-white/10 bg-black/30">
              <div>
                <div className="text-sm font-semibold text-white">Structured logs</div>
                <div className="text-xs text-zinc-500">Write audit entries as JSON lines locally for observability.</div>
              </div>
              <label className="inline-flex items-center cursor-pointer">
                <input
                  type="checkbox"
                  className="sr-only peer"
                  checked={structuredLogs}
                  onChange={(e) => setStructuredLogs(e.target.checked)}
                />
                <div className="w-11 h-6 bg-zinc-700 peer-focus:outline-none rounded-full peer peer-checked:bg-indigo-500 relative transition-colors">
                  <div className={`absolute top-1 left-1 w-4 h-4 bg-white rounded-full transition-transform ${structuredLogs ? "translate-x-5" : ""}`}></div>
                </div>
              </label>
            </div>
          </div>

          {/* Accessibility toggles */}
          <div className="grid grid-cols-1 gap-2">
            <div className="flex items-center justify-between px-3 py-3 rounded-xl border border-white/10 bg-black/30">
              <div>
                <div className="text-sm font-semibold text-white">Reduced motion</div>
                <div className="text-xs text-zinc-500">Trim animations and transitions.</div>
              </div>
              <label className="inline-flex items-center cursor-pointer">
                <input
                  type="checkbox"
                  className="sr-only peer"
                  checked={reducedMotion}
                  onChange={(e) => setReducedMotion(e.target.checked)}
                />
                <div className="w-11 h-6 bg-zinc-700 peer-focus:outline-none rounded-full peer peer-checked:bg-indigo-500 relative transition-colors">
                  <div className={`absolute top-1 left-1 w-4 h-4 bg-white rounded-full transition-transform ${reducedMotion ? "translate-x-5" : ""}`}></div>
                </div>
              </label>
            </div>

            <div className="flex items-center justify-between px-3 py-3 rounded-xl border border-white/10 bg-black/30">
              <div>
                <div className="text-sm font-semibold text-white">High contrast</div>
                <div className="text-xs text-zinc-500">Boost contrast for readability.</div>
              </div>
              <label className="inline-flex items-center cursor-pointer">
                <input
                  type="checkbox"
                  className="sr-only peer"
                  checked={highContrast}
                  onChange={(e) => setHighContrast(e.target.checked)}
                />
                <div className="w-11 h-6 bg-zinc-700 peer-focus:outline-none rounded-full peer peer-checked:bg-indigo-500 relative transition-colors">
                  <div className={`absolute top-1 left-1 w-4 h-4 bg-white rounded-full transition-transform ${highContrast ? "translate-x-5" : ""}`}></div>
                </div>
              </label>
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="bg-zinc-950/50 p-4 border-t border-white/5 flex justify-end">
          <button
            onClick={handleSave}
            disabled={loading}
            className="flex items-center gap-2 bg-indigo-600 hover:bg-indigo-500 text-white px-5 py-2.5 rounded-xl text-sm font-medium transition-all shadow-lg shadow-indigo-500/20 disabled:opacity-50 disabled:cursor-not-allowed hover:scale-[1.02] active:scale-[0.98]"
          >
            <Save className="w-4 h-4" />
            {loading ? "Saving..." : "Save Changes"}
          </button>
        </div>
      </div>
    </div>
  );
}
