import { useEffect } from 'react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Plus } from 'lucide-react'
import { useAopStore } from '@/store/aop-store'
import { getTasks } from '@/hooks/useTauri'
import TaskGraph from '@/components/TaskGraph'

export function TasksView() {
  const tasks = useAopStore((state) => Array.from(state.tasks.values()))
  const selectedTaskId = useAopStore((state) => state.selectedTaskId)
  const selectTask = useAopStore((state) => state.selectTask)

  useEffect(() => {
    // Load tasks only once on mount
    getTasks().then((fetchedTasks) => {
      const addTask = useAopStore.getState().addTask
      fetchedTasks.forEach((task) => addTask(task))
    })
  }, [])

  return (
    <div className="grid grid-cols-1 lg:grid-cols-[1fr_400px] gap-4">
      {/* Left: Task Graph */}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle>Task Hierarchy</CardTitle>
          <Button size="sm">
            <Plus className="w-4 h-4 mr-2" />
            New Task
          </Button>
        </CardHeader>
        <CardContent>
          <TaskGraph
            tasks={tasks}
            selectedTaskId={selectedTaskId}
            onTaskClick={selectTask}
            onTaskDoubleClick={(taskId) => console.log('Double clicked:', taskId)}
          />
        </CardContent>
      </Card>

      {/* Right: Task Details */}
      <Card>
        <CardHeader>
          <CardTitle>Task Details</CardTitle>
        </CardHeader>
        <CardContent>
          {selectedTaskId ? (
            <div>Task {selectedTaskId} details here</div>
          ) : (
            <p className="text-muted-foreground text-sm">Select a task to view details</p>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
