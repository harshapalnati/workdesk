import { useEffect, useState, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { Loader2, CheckCircle2, XCircle, Terminal, FileText, FolderOpen, ListTodo, Layers, Cpu, ChevronDown, ChevronRight } from "lucide-react";
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

function Section({ 
  title, 
  icon: Icon, 
  children, 
  defaultOpen = true,
  className = "",
  rightElement
}: { 
  title: string, 
  icon: any, 
  children: React.ReactNode, 
  defaultOpen?: boolean,
  className?: string,
  rightElement?: React.ReactNode
}) {
  const [isOpen, setIsOpen] = useState(defaultOpen);

  return (
    <div className={`flex flex-col border-b border-white/5 ${isOpen ? className : ''} transition-all duration-300 ease-in-out`}>
      <button 
        onClick={() => setIsOpen(!isOpen)}
        className="px-4 py-2.5 bg-zinc-900/40 flex items-center justify-between hover:bg-white/5 transition-colors group shrink-0"
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

export function RightSidebar({ workingDir }: { workingDir: string | null }) {
  const [activities, setActivities] = useState<ActivityEvent[]>([]);
  const [plan, setPlan] = useState<PlanEvent | null>(null);
  const [telemetry, setTelemetry] = useState<TelemetryEvent[]>([]);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
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
    <div className="flex flex-col h-full bg-zinc-900/30 border-l border-white/5 w-72 backdrop-blur-md overflow-hidden">
      
      {/* 1. Tasks / Plan Section */}
      <Section 
        title="Tasks" 
        icon={ListTodo} 
        className="flex-[0_0_35%]"
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

      {/* 2. Files Section */}
      <Section title="Files" icon={Layers} className="flex-1">
        <div className="flex-1 overflow-y-auto">
           <FileTree path={workingDir} />
        </div>
      </Section>

      {/* 3. Agents / Activity Section */}
      <Section title="Agents" icon={Cpu} className="flex-[0_0_30%]">
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

      {/* 4. Telemetry */}
      <Section title="Telemetry" icon={Terminal} className="flex-[0_0_25%]">
        <div className="flex-1 overflow-y-auto p-3 space-y-2 bg-zinc-900/10">
          {telemetry.length === 0 ? (
            <div className="text-xs text-zinc-600 italic text-center py-2">No telemetry yet.</div>
          ) : (
            telemetry.slice(-15).reverse().map((t, idx) => (
              <div key={idx} className="text-[11px] text-zinc-400 flex justify-between gap-2">
                <span className="truncate">{t.tool}</span>
                <span className={`text-xs ${t.status === "success" ? "text-emerald-400" : "text-red-400"}`}>{t.status}</span>
                <span className="text-zinc-500">{t.duration_ms}ms</span>
              </div>
            ))
          )}
        </div>
      </Section>

    </div>
  );
}

