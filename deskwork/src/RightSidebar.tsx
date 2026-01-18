import { useEffect, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Loader2, CheckCircle2, XCircle, Terminal, FileText, FolderOpen, ListTodo, Layers, Cpu, ChevronDown, ChevronRight, GripHorizontal, LayoutTemplate } from "lucide-react";
import { FileTree } from "./FileTree";

interface ActivityEvent {
  id: string;
  status: "pending" | "running" | "success" | "error";
  message: string;
  timestamp: number;
}

interface PlanEvent {
  steps: string[]; 
  current_step: number; 
}

interface TelemetryEvent {
  tool: string;
  status: string;
  duration_ms: number;
  kind: string;
}

interface Skill {
  id: string;
  name: string;
  description: string;
  enabled: boolean;
}

function Section({ 
  title, 
  icon: Icon, 
  children, 
  isOpen,
  onToggle,
  style,
  rightElement
}: { 
  title: string, 
  icon: any, 
  children: React.ReactNode, 
  isOpen: boolean,
  onToggle: () => void,
  style?: React.CSSProperties,
  rightElement?: React.ReactNode
}) {
  return (
    <div 
      style={style}
      className={`flex flex-col border-b border-white/5 transition-all duration-75 ease-out ${isOpen ? 'min-h-[100px]' : ''}`}
    >
      <button 
        onClick={onToggle}
        className="px-4 py-2.5 bg-zinc-900/40 flex items-center justify-between hover:bg-white/5 transition-colors group shrink-0 select-none"
      >
        <div className="flex items-center gap-2">
           <Icon className="w-3.5 h-3.5 text-zinc-500 group-hover:text-zinc-300" />
           <span className="text-[10px] font-bold text-zinc-500 group-hover:text-zinc-300 uppercase tracking-wider transition-colors">{title}</span>
        </div>
        <div className="flex items-center gap-2">
          {rightElement}
          {isOpen ? <ChevronDown className="w-3 h-3 text-zinc-600" /> : <ChevronRight className="w-3 h-3 text-zinc-600" />}
        </div>
      </button>
      {isOpen && (
        <div className="flex-1 overflow-hidden flex flex-col min-h-0">
          {children}
        </div>
      )}
    </div>
  );
}

function ResizeHandle({ onResize }: { onResize: (dy: number) => void }) {
  const [isDragging, setIsDragging] = useState(false);
  const startY = useRef<number>(0);

  useEffect(() => {
    if (!isDragging) return;

    const handleMove = (e: PointerEvent) => {
      const dy = e.clientY - startY.current;
      onResize(dy);
      startY.current = e.clientY; // Reset for relative delta
    };

    const handleUp = () => {
      setIsDragging(false);
      document.body.style.cursor = 'default';
    };

    window.addEventListener('pointermove', handleMove);
    window.addEventListener('pointerup', handleUp);

    return () => {
      window.removeEventListener('pointermove', handleMove);
      window.removeEventListener('pointerup', handleUp);
    };
  }, [isDragging, onResize]);

  return (
    <div 
      className={`h-1.5 -my-0.5 cursor-ns-resize z-10 hover:bg-indigo-500/50 transition-colors flex items-center justify-center group ${isDragging ? 'bg-indigo-500/50' : 'bg-transparent'}`}
      onPointerDown={(e) => {
        setIsDragging(true);
        startY.current = e.clientY;
        document.body.style.cursor = 'ns-resize';
      }}
    >
       <div className={`w-8 h-0.5 rounded-full bg-zinc-700/50 group-hover:bg-indigo-400/50 ${isDragging ? 'bg-indigo-400' : ''}`} />
    </div>
  );
}

export function RightSidebar({ workingDir }: { workingDir: string | null }) {
  const [activities, setActivities] = useState<ActivityEvent[]>([]);
  const [plan, setPlan] = useState<PlanEvent | null>(null);
  const [telemetry, setTelemetry] = useState<TelemetryEvent[]>([]);
  const [skills, setSkills] = useState<Skill[]>([]);
  const scrollRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Layout State
  // 0: Tasks, 1: Files, 2: Skills, 3: Agents
  const [sizes, setSizes] = useState([25, 25, 25, 25]); // Percentages
  const [openSections, setOpenSections] = useState([true, true, true, true]);

  const toggleSection = (idx: number) => {
    setOpenSections(prev => {
      const next = [...prev];
      next[idx] = !next[idx];
      return next;
    });
  };

  const handleResize = (idx: number, dy: number) => {
    // idx is the index of the SECTION ABOVE the handle.
    // We want to resize sizes[idx] vs sizes[idx+1]
    // dy is in pixels. We need to convert to percentage of CONTAINER height.
    if (!containerRef.current) return;
    
    const containerHeight = containerRef.current.clientHeight;
    const deltaPercent = (dy / containerHeight) * 100;

    setSizes(prev => {
      const next = [...prev];
      // Simple constraint: don't let sections get too small (e.g. < 5%)
      // Also ensure we are trading space between two OPEN sections if possible?
      // For simplicity, we just trade between the adjacent defined sizes, 
      // even if they are closed (which just updates their "potential" size).
      
      const newUpper = next[idx] + deltaPercent;
      const newLower = next[idx + 1] - deltaPercent;

      if (newUpper > 5 && newLower > 5) {
        next[idx] = newUpper;
        next[idx + 1] = newLower;
      }
      return next;
    });
  };

  useEffect(() => {
    loadSkills();
    const unlistenActivity = listen<ActivityEvent>("activity", (event) => {
      setActivities((prev) => {
        const index = prev.findIndex((a) => a.id === event.payload.id);
        if (index >= 0) {
          const newActivities = [...prev];
          newActivities[index] = { ...event.payload, timestamp: Date.now() };
          return newActivities;
        } else {
          return [...prev, { ...event.payload, timestamp: Date.now() }];
        }
      });
    });

    const unlistenPlan = listen<PlanEvent>("plan_update", (event) => {
      setPlan(event.payload);
    });

    const unlistenTelemetry = listen<TelemetryEvent>("telemetry", (event) => {
      setTelemetry((prev) => {
        const next = [...prev, event.payload];
        return next.slice(-20);
      });
    });

    return () => {
      unlistenActivity.then((f) => f());
      unlistenPlan.then((f) => f());
      unlistenTelemetry.then((f) => f());
    };
  }, []);

  async function loadSkills() {
    try {
      const list = await invoke<Skill[]>("list_skills");
      setSkills(list);
    } catch (e) {
      console.error("Failed to load skills", e);
    }
  }

  async function toggleSkill(id: string, enabled: boolean) {
    try {
      await invoke("toggle_skill", { id, enabled });
      setSkills(prev => prev.map(s => s.id === id ? { ...s, enabled } : s));
    } catch (e) {
      console.error("Failed to toggle skill", e);
    }
  }

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [activities]);

  const getIcon = (status: string, message: string) => {
    if (status === "running" || status === "pending") return <Loader2 className="w-3.5 h-3.5 animate-spin text-indigo-400" />;
    if (status === "error") return <XCircle className="w-3.5 h-3.5 text-red-400" />;
    return <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />;
  };

  return (
    <div ref={containerRef} className="flex flex-col h-full bg-zinc-900/30 border-l border-white/5 w-72 backdrop-blur-md overflow-hidden">
      
      {/* 1. Tasks / Plan Section */}
      <Section 
        title="Tasks" 
        icon={ListTodo} 
        isOpen={openSections[0]}
        onToggle={() => toggleSection(0)}
        style={{ flex: openSections[0] ? `${sizes[0]} 1 0%` : 'none' }}
        rightElement={plan && (
             <span className="text-[10px] text-zinc-500 font-mono">
               {plan.current_step}/{plan.steps.length}
             </span>
        )}
      >
        <div className="flex-1 overflow-y-auto p-4 space-y-3">
          {!plan ? (
            <div className="text-xs text-zinc-600 italic text-center py-2">
              No active plan.
            </div>
          ) : (
            plan.steps.map((step, idx) => {
              const isCompleted = idx < plan.current_step;
              const isCurrent = idx === plan.current_step;
              
              return (
                <div key={idx} className="flex items-start gap-3">
                  <div className="mt-0.5 shrink-0">
                    {isCompleted ? (
                      <CheckCircle2 className="w-4 h-4 text-indigo-500" />
                    ) : isCurrent ? (
                      <div className="w-4 h-4 rounded-full border-2 border-indigo-500 flex items-center justify-center animate-pulse">
                         <div className="w-1.5 h-1.5 bg-indigo-500 rounded-full" />
                      </div>
                    ) : (
                      <div className="w-4 h-4 rounded-full border border-zinc-700 bg-zinc-800" />
                    )}
                  </div>
                  <p className={`text-xs font-medium leading-tight ${isCompleted ? 'text-zinc-500 line-through' : isCurrent ? 'text-zinc-200' : 'text-zinc-600'}`}>
                    {step}
                  </p>
                </div>
              );
            })
          )}
        </div>
      </Section>
      
      {openSections[0] && openSections[1] && <ResizeHandle onResize={(dy) => handleResize(0, dy)} />}

      {/* 2. Files Section */}
      <Section 
        title="Files" 
        icon={Layers} 
        isOpen={openSections[1]}
        onToggle={() => toggleSection(1)}
        style={{ flex: openSections[1] ? `${sizes[1]} 1 0%` : 'none' }}
      >
        <div className="flex-1 overflow-y-auto">
           <FileTree path={workingDir} />
        </div>
      </Section>

      {openSections[1] && openSections[2] && <ResizeHandle onResize={(dy) => handleResize(1, dy)} />}

      {/* 3. Skills Section (New) */}
      <Section 
        title="Skills" 
        icon={LayoutTemplate} 
        isOpen={openSections[2]}
        onToggle={() => toggleSection(2)}
        style={{ flex: openSections[2] ? `${sizes[2]} 1 0%` : 'none' }}
      >
        <div className="flex-1 overflow-y-auto p-2 space-y-1">
          {skills.map((skill) => (
            <div key={skill.id} className="flex items-center justify-between px-3 py-2 bg-white/5 rounded-lg border border-white/5 hover:border-white/10 transition-colors">
              <div className="min-w-0 flex-1 mr-3">
                <div className="text-xs font-medium text-zinc-200 truncate">{skill.name}</div>
                <div className="text-[10px] text-zinc-500 truncate">{skill.description}</div>
              </div>
              <label className="relative inline-flex items-center cursor-pointer shrink-0">
                <input 
                  type="checkbox" 
                  className="sr-only peer" 
                  checked={skill.enabled}
                  onChange={(e) => toggleSkill(skill.id, e.target.checked)}
                />
                <div className="w-7 h-4 bg-zinc-700 peer-focus:outline-none rounded-full peer peer-checked:bg-indigo-500 after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-3 after:w-3 after:transition-all peer-checked:after:translate-x-full peer-checked:after:border-white"></div>
              </label>
            </div>
          ))}
        </div>
      </Section>

      {openSections[2] && openSections[3] && <ResizeHandle onResize={(dy) => handleResize(2, dy)} />}

      {/* 4. Agents / Activity Section */}
      <Section 
        title="Agents" 
        icon={Cpu} 
        isOpen={openSections[3]}
        onToggle={() => toggleSection(3)}
        style={{ flex: openSections[3] ? `${sizes[3]} 1 0%` : 'none' }}
      >
        <div ref={scrollRef} className="flex-1 overflow-y-auto p-3 space-y-2 bg-zinc-900/20">
          {activities.length === 0 ? (
             <div className="text-xs text-zinc-600 italic text-center py-2">
               Agent is idle.
             </div>
          ) : (
            activities.slice(-20).map((activity, idx) => ( 
            <div key={`${activity.id}-${idx}`} className="flex items-start gap-2 animate-in slide-in-from-right-2 fade-in duration-300">
               <div className="mt-0.5 shrink-0">
                 {getIcon(activity.status, activity.message)}
               </div>
               <p className="text-[10px] text-zinc-400 leading-snug break-words">
                 {activity.message}
               </p>
            </div>
          )))}
        </div>
      </Section>

    </div>
  );
}

