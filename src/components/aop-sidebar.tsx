import * as React from 'react'
import {
  Target,
  LayoutDashboard,
  Brain,
  GitPullRequest,
  FileText,
  Settings,
  Activity,
} from 'lucide-react'

import { NavMain } from '@/components/nav-main'
import { NavSecondary } from '@/components/nav-secondary'
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from '@/components/ui/sidebar'
import { useAopStore } from '@/store/aop-store'
import type { AppTab } from '@/store/types'

const navItems = [
  {
    title: 'Tasks',
    url: '#',
    icon: Target,
    value: 'tasks' as AppTab,
  },
  {
    title: 'Dashboard',
    url: '#',
    icon: LayoutDashboard,
    value: 'dashboard' as AppTab,
  },
  {
    title: 'Context',
    url: '#',
    icon: Brain,
    value: 'context' as AppTab,
  },
  {
    title: 'Mutations',
    url: '#',
    icon: GitPullRequest,
    value: 'mutations' as AppTab,
  },
  {
    title: 'Logs',
    url: '#',
    icon: FileText,
    value: 'logs' as AppTab,
  },
]

const secondaryItems = [
  {
    title: 'System',
    url: '#',
    icon: Settings,
    value: 'system' as AppTab,
  },
]

export function AopSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
  const { activeTab, setActiveTab } = useAopStore()

  const handleNavClick = (value: AppTab) => (e: React.MouseEvent) => {
    e.preventDefault()
    setActiveTab(value)
  }

  return (
    <Sidebar collapsible="icon" {...props}>
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              asChild
              className="data-[slot=sidebar-menu-button]:!p-1.5"
            >
              <a href="#" onClick={(e) => e.preventDefault()}>
                <Activity className="!size-5" />
                <span className="text-base font-semibold">AOP</span>
              </a>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>
      <SidebarContent>
        <NavMain
          items={navItems.map((item) => ({
            ...item,
            isActive: activeTab === item.value,
            onClick: handleNavClick(item.value),
          }))}
        />
        <NavSecondary
          items={secondaryItems.map((item) => ({
            ...item,
            isActive: activeTab === item.value,
            onClick: handleNavClick(item.value),
          }))}
          className="mt-auto"
        />
      </SidebarContent>
      <SidebarFooter>
        <div className="p-4 text-xs text-muted-foreground">
          AI Orchestration Platform
        </div>
      </SidebarFooter>
    </Sidebar>
  )
}
