import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { useAopStore } from '@/store/aop-store'
import type { AppTab } from '@/store/types'
import {
  Target,
  LayoutDashboard,
  Brain,
  GitPullRequest,
  FileText,
  Settings,
} from 'lucide-react'

const TAB_CONFIG: Array<{ id: AppTab; label: string; icon: React.ReactNode }> = [
  { id: 'tasks', label: 'Tasks', icon: <Target className="w-4 h-4" /> },
  { id: 'dashboard', label: 'Dashboard', icon: <LayoutDashboard className="w-4 h-4" /> },
  { id: 'context', label: 'Context', icon: <Brain className="w-4 h-4" /> },
  { id: 'mutations', label: 'Mutations', icon: <GitPullRequest className="w-4 h-4" /> },
  { id: 'logs', label: 'Logs', icon: <FileText className="w-4 h-4" /> },
  { id: 'system', label: 'System', icon: <Settings className="w-4 h-4" /> },
]

export function TabBar() {
  const { activeTab, setActiveTab } = useAopStore()

  return (
    <Tabs value={activeTab} onValueChange={(value: string) => setActiveTab(value as AppTab)}>
      <TabsList className="grid w-full grid-cols-6">
        {TAB_CONFIG.map((tab) => (
          <TabsTrigger key={tab.id} value={tab.id} className="flex items-center gap-2">
            {tab.icon}
            <span className="hidden sm:inline">{tab.label}</span>
          </TabsTrigger>
        ))}
      </TabsList>
    </Tabs>
  )
}
