import { AopSidebar } from "@/components/aop-sidebar";
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar";
import { Separator } from "@/components/ui/separator";
import { useAopStore } from "@/store/aop-store";
import { TasksView } from "@/views/TasksView";
import { DashboardView } from "@/views/DashboardView";
import { ContextView } from "@/views/ContextView";
import { MutationsView } from "@/views/MutationsView";
import { TerminalView } from "@/views/TerminalView";
import { LogsView } from "@/views/LogsView";
import { SystemView } from "@/views/SystemView";

export function AppLayout() {
  const activeTab = useAopStore((state) => state.activeTab);

  const viewTitles = {
    tasks: "Task Hierarchy",
    dashboard: "Dashboard",
    context: "Semantic Context",
    mutations: "Mutation Pipeline",
    terminal: "Agent Terminals",
    logs: "System Logs",
    system: "System Health",
  };

  return (
    <SidebarProvider
      style={
        {
          "--sidebar-width": "calc(var(--spacing) * 64)",
          "--header-height": "calc(var(--spacing) * 12)",
        } as React.CSSProperties
      }
    >
      <AopSidebar variant="inset" />
      <SidebarInset>
        <header className="flex h-(--header-height) shrink-0 items-center gap-2 border-b transition-[width,height] ease-linear group-has-data-[collapsible=icon]/sidebar-wrapper:h-(--header-height)">
          <div className="flex w-full items-center gap-1 px-4 lg:gap-2 lg:px-6">
            <SidebarTrigger className="-ml-1" />
            <Separator
              orientation="vertical"
              className="mx-2 data-[orientation=vertical]:h-4"
            />
            <h1 className="text-base font-medium">{viewTitles[activeTab]}</h1>
          </div>
        </header>
        <main className="flex flex-1 flex-col p-6">
          {activeTab === "dashboard" && <DashboardView />}
          {activeTab === "tasks" && <TasksView />}
          {activeTab === "context" && <ContextView />}
          {activeTab === "mutations" && <MutationsView />}
          {activeTab === "terminal" && <TerminalView />}
          {activeTab === "logs" && <LogsView />}
          {activeTab === "system" && <SystemView />}
        </main>
      </SidebarInset>
    </SidebarProvider>
  );
}
