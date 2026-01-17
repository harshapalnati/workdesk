import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Folder, FileText, ChevronRight, ChevronDown, RefreshCw } from "lucide-react";

interface FileNode {
  name: string;
  path: string;
  is_dir: boolean;
  children?: FileNode[];
}

export function FileTree({ path }: { path: string | null }) {
  const [tree, setTree] = useState<FileNode[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (path) {
      loadTree();
    }
  }, [path]);

  async function loadTree() {
    if (!path) return;
    setLoading(true);
    try {
      const nodes = await invoke<FileNode[]>("get_file_tree", { path });
      setTree(nodes);
    } catch (e) {
      console.error("Failed to load file tree", e);
    } finally {
      setLoading(false);
    }
  }

  if (!path) {
    return (
      <div className="p-4 text-xs text-zinc-500 text-center italic">
        Select a folder to see files.
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center justify-between px-4 py-2 border-b border-white/5 bg-zinc-900/30">
        <span className="text-[10px] font-semibold text-zinc-500 uppercase tracking-wider">
          Files
        </span>
        <button onClick={loadTree} className="text-zinc-500 hover:text-white transition-colors">
          <RefreshCw className={`w-3 h-3 ${loading ? "animate-spin" : ""}`} />
        </button>
      </div>
      <div className="flex-1 overflow-y-auto p-2">
        {tree.map((node) => (
          <FileTreeNode key={node.path} node={node} level={0} />
        ))}
      </div>
    </div>
  );
}

function FileTreeNode({ node, level }: { node: FileNode; level: number }) {
  const [isOpen, setIsOpen] = useState(false);
  const hasChildren = node.children && node.children.length > 0;

  return (
    <div>
      <div 
        className="flex items-center gap-1.5 py-1 px-2 rounded-md hover:bg-white/5 cursor-pointer text-zinc-400 hover:text-zinc-200 transition-colors"
        style={{ paddingLeft: `${level * 12 + 8}px` }}
        onClick={() => hasChildren && setIsOpen(!isOpen)}
      >
        {node.is_dir ? (
          <>
            <span className="shrink-0 text-zinc-600">
              {isOpen ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
            </span>
            <Folder className="w-3.5 h-3.5 shrink-0 text-indigo-400/70" />
          </>
        ) : (
          <>
            <span className="w-3 shrink-0" /> {/* Spacer for alignment */}
            <FileText className="w-3.5 h-3.5 shrink-0 text-zinc-600" />
          </>
        )}
        <span className="text-xs truncate select-none">{node.name}</span>
      </div>
      
      {isOpen && node.children && (
        <div>
          {node.children.map((child) => (
            <FileTreeNode key={child.path} node={child} level={level + 1} />
          ))}
        </div>
      )}
    </div>
  );
}

