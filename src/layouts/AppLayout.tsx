import { TabBar } from '@/components/navigation/TabBar'
import { useAopStore } from '@/store/aop-store'
import { TasksView } from '@/views/TasksView'
import { DashboardView } from '@/views/DashboardView'
import { ContextView } from '@/views/ContextView'
import { MutationsView } from '@/views/MutationsView'
import { LogsView } from '@/views/LogsView'
import { SystemView } from '@/views/SystemView'

export function AppLayout() {
  const activeTab = useAopStore((state) => state.activeTab)

  return (
    <div className="min-h-screen bg-background">
      <header className="border-b border-border bg-card/50 backdrop-blur-sm sticky top-0 z-50">
        <div className="container mx-auto px-4 py-3">
          <div className="flex items-center justify-between mb-3">
            <div>
              <h1 className="text-2xl font-bold tracking-tight">AOP</h1>
              <p className="text-sm text-muted-foreground">AI Orchestration Platform</p>
            </div>
            <div className="flex items-center gap-2">
              {/* Project selector & settings will go here */}
            </div>
          </div>
          <TabBar />
        </div>
      </header>

      <main className="container mx-auto px-4 py-6">
        {activeTab === 'tasks' && <TasksView />}
        {activeTab === 'dashboard' && <DashboardView />}
        {activeTab === 'context' && <ContextView />}
        {activeTab === 'mutations' && <MutationsView />}
        {activeTab === 'logs' && <LogsView />}
        {activeTab === 'system' && <SystemView />}
      </main>
    </div>
  )
}
