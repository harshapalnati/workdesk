import { useEffect, useState, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { Loader2, CheckCircle2, XCircle, Terminal, FileText, FolderOpen, ListTodo } from "lucide-react";

interface ActivityEvent {
  id: string;
  status: "pending" | "running" | "success" | "error";
  message: string;
  timestamp: number;
}

interface PlanEvent {
  steps: string[];
  current_step: number; // 0-indexed index of the step currently being worked on (or next to be worked on)
}

export function ActivityStream() {
  const [activities, setActivities] = useState<ActivityEvent[]>([]);
  const [plan, setPlan] = useState<PlanEvent | null>(null);
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

    return () => {
      unlistenActivity.then((f) => f());
      unlistenPlan.then((f) => f());
    };
  }, []);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [activities]);

  const getIcon = (status: string, message: string) => {
    if (status === "running" || status === "pending") return <Loader2 className="w-4 h-4 animate-spin text-indigo-400" />;
    if (status === "error") return <XCircle className="w-4 h-4 text-red-400" />;
    if (message.includes("File")) return <FileText className="w-4 h-4 text-emerald-400" />;
    if (message.includes("Listing")) return <FolderOpen className="w-4 h-4 text-emerald-400" />;
    if (message.includes("Executing")) return <Terminal className="w-4 h-4 text-emerald-400" />;
    return <CheckCircle2 className="w-4 h-4 text-emerald-400" />;
  };

  return (
    <div className="flex flex-col h-full bg-zinc-900/30 border-l border-white/5 w-80 backdrop-blur-md">
      {/* Plan Section */}
      <div className="p-5 border-b border-white/5 bg-zinc-900/40">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-sm font-bold text-white tracking-wide flex items-center gap-2">
            <ListTodo className="w-4 h-4 text-indigo-400" />
            Progress
          </h2>
        </div>

        {!plan ? (
          <div className="text-xs text-zinc-500 italic px-1">
            No active plan. Ask me to do a task!
          </div>
        ) : (
          <div className="space-y-3">
            {plan.steps.map((step, idx) => {
              const isCompleted = idx < plan.current_step;
              const isCurrent = idx === plan.current_step;
              const isPending = idx > plan.current_step;

              return (
                <div key={idx} className={`flex items-start gap-3 transition-all duration-300 ${isPending ? 'opacity-50' : 'opacity-100'}`}>
                  {/* Status Icon/Number */}
                  <div className="mt-0.5 shrink-0">
                    {isCompleted ? (
                      <div className="w-5 h-5 rounded-full bg-indigo-500 flex items-center justify-center shadow-lg shadow-indigo-500/20">
                        <CheckCircle2 className="w-3.5 h-3.5 text-white" />
                      </div>
                    ) : isCurrent ? (
                      <div className="w-5 h-5 rounded-full border-2 border-indigo-500 flex items-center justify-center animate-pulse">
                         <span className="text-[10px] font-bold text-indigo-400">{idx + 1}</span>
                      </div>
                    ) : (
                      <div className="w-5 h-5 rounded-full border border-zinc-700 bg-zinc-800 flex items-center justify-center">
                        <span className="text-[10px] font-medium text-zinc-500">{idx + 1}</span>
                      </div>
                    )}
                  </div>
                  
                  {/* Step Text */}
                  <p className={`text-xs font-medium leading-5 ${
                    isCompleted ? 'text-zinc-400 line-through' : 
                    isCurrent ? 'text-white' : 
                    'text-zinc-500'
                  }`}>
                    {step}
                  </p>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Activity Log Header */}
      <div className="px-4 py-2 bg-zinc-950/30 border-b border-white/5">
        <h3 className="text-[10px] font-semibold text-zinc-500 uppercase tracking-wider">
          System Activity
        </h3>
      </div>

      {/* Activity Log List */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto p-4 space-y-3">
        {activities.map((activity, idx) => (
          <div 
            key={`${activity.id}-${idx}`}
            className="group relative pl-4 border-l border-zinc-800 last:border-0 animate-in slide-in-from-right-2 duration-300"
          >
            <div className="absolute -left-[4.5px] top-1.5 w-2 h-2 rounded-full bg-zinc-800 border border-zinc-600 group-hover:border-indigo-500 transition-colors"></div>
            
            <div className="flex items-start gap-2.5 opacity-80 group-hover:opacity-100 transition-opacity">
              <div className="mt-0.5 shrink-0">
                {getIcon(activity.status, activity.message)}
              </div>
              <div className="min-w-0 flex-1">
                <p className="text-[11px] font-medium text-zinc-400 break-words leading-tight">
                  {activity.message}
                </p>
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
