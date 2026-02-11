import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type FormEvent,
} from "react";

import TaskGraph from "@/components/TaskGraph";
import TaskActivityFeed from "@/components/TaskActivityFeed";
import TaskBudgetPanel from "@/components/TaskBudgetPanel";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { useTargetProjectConfig } from "@/hooks/useTargetProjectConfig";
import { controlTask, createTask, executeDomainTask, getTasks } from "@/hooks/useTauri";
import { useAopStore } from "@/store/aop-store";
import type { AppTab } from "@/store/types";
import type { CreateTaskInput, TaskControlAction, TaskRecord } from "@/types";
import { Plus } from "lucide-react";

const DEFAULT_TASK_FORM: CreateTaskInput = {
  parentId: null,
  tier: 1,
  domain: "platform",
  objective: "",
  tokenBudget: 3000,
};

function formatTimestamp(timestamp: number): string {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(timestamp * 1000));
}

export function TasksView() {
  const tasksMap = useAopStore((state) => state.tasks);
  const tasks = useMemo<TaskRecord[]>(
    () =>
      Array.from<TaskRecord>(tasksMap.values()).sort(
        (left: TaskRecord, right: TaskRecord) =>
          right.createdAt - left.createdAt,
      ),
    [tasksMap],
  );
  const selectedTaskId = useAopStore((state) => state.selectedTaskId);
  const selectTask = useAopStore((state) => state.selectTask);
  const addTask = useAopStore((state) => state.addTask);
  const setActiveTab = useAopStore((state) => state.setActiveTab);
  const { targetProject, mcpConfig } = useTargetProjectConfig();

  const selectedTask: TaskRecord | null =
    tasks.find((task) => task.id === selectedTaskId) ?? null;
  const parentByTaskId = useMemo(() => {
    const result = new Map<string, string | null>();
    tasks.forEach((task) => {
      result.set(task.id, task.parentId);
    });
    return result;
  }, [tasks]);
  const resumableTaskIds = useMemo(() => {
    const result = new Set<string>();
    tasks.forEach((task) => {
      if (task.status !== "paused") {
        return;
      }

      let currentId: string | null = task.id;
      while (currentId) {
        result.add(currentId);
        currentId = parentByTaskId.get(currentId) ?? null;
      }
    });
    return result;
  }, [parentByTaskId, tasks]);
  const pausableTaskIds = useMemo(() => {
    const result = new Set<string>();
    tasks.forEach((task) => {
      if (
        task.status === "completed" ||
        task.status === "failed" ||
        task.status === "paused"
      ) {
        return;
      }

      let currentId: string | null = task.id;
      while (currentId) {
        result.add(currentId);
        currentId = parentByTaskId.get(currentId) ?? null;
      }
    });
    return result;
  }, [parentByTaskId, tasks]);
  const stoppableTaskIds = useMemo(() => {
    const result = new Set<string>();
    tasks.forEach((task) => {
      if (task.status === "completed" || task.status === "failed") {
        return;
      }

      let currentId: string | null = task.id;
      while (currentId) {
        result.add(currentId);
        currentId = parentByTaskId.get(currentId) ?? null;
      }
    });
    return result;
  }, [parentByTaskId, tasks]);
  const restartableTaskIds = useMemo(() => {
    const result = new Set<string>();
    tasks.forEach((task) => {
      if (
        task.status !== "failed" &&
        task.status !== "completed" &&
        task.status !== "paused"
      ) {
        return;
      }

      let currentId: string | null = task.id;
      while (currentId) {
        result.add(currentId);
        currentId = parentByTaskId.get(currentId) ?? null;
      }
    });
    return result;
  }, [parentByTaskId, tasks]);
  const canPauseSelectedTask = selectedTask
    ? pausableTaskIds.has(selectedTask.id)
    : false;
  const canResumeSelectedTask = selectedTask
    ? resumableTaskIds.has(selectedTask.id)
    : false;
  const canStopSelectedTask = selectedTask
    ? stoppableTaskIds.has(selectedTask.id)
    : false;
  const canRestartSelectedTask = selectedTask
    ? restartableTaskIds.has(selectedTask.id)
    : false;

  const [isLoadingTasks, setIsLoadingTasks] = useState(false);
  const [taskLoadError, setTaskLoadError] = useState<string | null>(null);
  const [isCreateDialogOpen, setIsCreateDialogOpen] = useState(false);
  const [isCreatingTask, setIsCreatingTask] = useState(false);
  const [createTaskError, setCreateTaskError] = useState<string | null>(null);
  const [taskControlError, setTaskControlError] = useState<string | null>(null);
  const [activeTaskControl, setActiveTaskControl] =
    useState<TaskControlAction | null>(null);
  const [taskForm, setTaskForm] = useState<CreateTaskInput>(DEFAULT_TASK_FORM);

  const goToTab = useCallback(
    (tab: AppTab) => {
      setActiveTab(tab);
    },
    [setActiveTab],
  );

  const loadTasks = useCallback(async () => {
    setIsLoadingTasks(true);
    setTaskLoadError(null);
    try {
      const fetchedTasks = await getTasks();
      fetchedTasks.forEach((task) => addTask(task));
      if (!selectedTaskId && fetchedTasks.length > 0) {
        selectTask(fetchedTasks[0].id);
      }
    } catch (error) {
      setTaskLoadError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsLoadingTasks(false);
    }
  }, [addTask, selectTask, selectedTaskId]);

  useEffect(() => {
    void loadTasks();
  }, [loadTasks]);

  function resetCreateTaskState() {
    setCreateTaskError(null);
    setTaskForm((previous) => ({
      ...previous,
      objective: "",
    }));
  }

  async function handleCreateTask(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setCreateTaskError(null);

    const domain = taskForm.domain.trim();
    const objective = taskForm.objective.trim();
    const tokenBudget = Number(taskForm.tokenBudget);

    if (!domain) {
      setCreateTaskError("Domain is required.");
      return;
    }
    if (!objective) {
      setCreateTaskError("Objective is required.");
      return;
    }
    if (!Number.isFinite(tokenBudget) || tokenBudget <= 0) {
      setCreateTaskError("Token budget must be greater than 0.");
      return;
    }

    setIsCreatingTask(true);
    try {
      const createdTask = await createTask({
        parentId: null,
        tier: taskForm.tier,
        domain,
        objective,
        tokenBudget: Math.floor(tokenBudget),
      });
      addTask(createdTask);
      selectTask(createdTask.id);
      setIsCreateDialogOpen(false);
      resetCreateTaskState();
    } catch (error) {
      setCreateTaskError(
        error instanceof Error ? error.message : String(error),
      );
    } finally {
      setIsCreatingTask(false);
    }
  }

  async function handleTaskControl(action: TaskControlAction) {
    if (!selectedTask) {
      return;
    }

    setTaskControlError(null);
    setActiveTaskControl(action);
    try {
      const updated = await controlTask({
        taskId: selectedTask.id,
        action,
        includeDescendants: true,
        reason: action === "stop" ? "manual stop from task panel" : undefined,
      });
      if (updated.length === 0) {
        setTaskControlError(`No tasks were updated for action '${action}'.`);
      }
      updated.forEach((task) => addTask(task));

      if (action === "restart") {
        const target = targetProject.trim();
        if (!target) {
          setTaskControlError(
            "Tasks were restarted, but no target project is configured to run Tier 2 agents.",
          );
          await loadTasks();
          return;
        }

        const tier2TaskIds = Array.from(
          new Set(
            updated
              .filter((task) => task.tier === 2)
              .map((task) => task.id),
          ),
        );
        const executionResults = await Promise.allSettled(
          tier2TaskIds.map((taskId) =>
            executeDomainTask({
              taskId,
              targetProject: target,
              topK: 8,
              ...mcpConfig,
            }),
          ),
        );
        const failedExecutions = executionResults.filter(
          (result) => result.status === "rejected",
        );
        if (failedExecutions.length > 0) {
          const firstFailure = failedExecutions[0] as PromiseRejectedResult;
          const message =
            firstFailure.reason instanceof Error
              ? firstFailure.reason.message
              : String(firstFailure.reason);
          setTaskControlError(
            `Restarted tasks, but ${failedExecutions.length} Tier 2 execution(s) failed. First error: ${message}`,
          );
        }
      }

      await loadTasks();
    } catch (error) {
      setTaskControlError(
        error instanceof Error ? error.message : String(error),
      );
    } finally {
      setActiveTaskControl(null);
    }
  }

  return (
    <div className="grid grid-cols-1 gap-4 md:grid-cols-2!">
      <Card>
        <CardHeader>
          <CardTitle>Task Details</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          {selectedTask ? (
            <>
              <div className="space-y-1">
                <p className="text-sm font-semibold">{selectedTask.domain}</p>
                <p className="text-muted-foreground text-sm">
                  {selectedTask.objective}
                </p>
              </div>

              <div className="grid grid-cols-2 gap-2 text-xs">
                <div className="rounded-md border p-2">
                  Tier {selectedTask.tier}
                </div>
                <div className="rounded-md border p-2">
                  {selectedTask.status}
                </div>
                <div className="rounded-md border p-2">
                  Budget {selectedTask.tokenBudget}
                </div>
                <div className="rounded-md border p-2">
                  Used {selectedTask.tokenUsage}
                </div>
                <div className="rounded-md border p-2">
                  Compliance {selectedTask.complianceScore}
                </div>
                <div className="rounded-md border p-2">
                  Risk {selectedTask.riskFactor.toFixed(2)}
                </div>
              </div>

              <p className="text-muted-foreground text-xs">
                Created {formatTimestamp(selectedTask.createdAt)}
              </p>
              <p className="text-muted-foreground text-xs">
                Updated {formatTimestamp(selectedTask.updatedAt)}
              </p>

              <div className="flex flex-wrap gap-2">
                <Button
                  disabled={activeTaskControl !== null || !canPauseSelectedTask}
                  onClick={() => void handleTaskControl("pause")}
                  size="sm"
                  type="button"
                  variant="outline"
                >
                  {activeTaskControl === "pause" ? "Pausing..." : "Pause"}
                </Button>
                <Button
                  disabled={
                    activeTaskControl !== null || !canResumeSelectedTask
                  }
                  onClick={() => void handleTaskControl("resume")}
                  size="sm"
                  type="button"
                  variant="outline"
                >
                  {activeTaskControl === "resume" ? "Resuming..." : "Resume"}
                </Button>
                <Button
                  disabled={activeTaskControl !== null || !canStopSelectedTask}
                  onClick={() => void handleTaskControl("stop")}
                  size="sm"
                  type="button"
                  variant="destructive"
                >
                  {activeTaskControl === "stop" ? "Stopping..." : "Stop"}
                </Button>
                <Button
                  disabled={activeTaskControl !== null || !canRestartSelectedTask}
                  onClick={() => void handleTaskControl("restart")}
                  size="sm"
                  type="button"
                  variant="secondary"
                >
                  {activeTaskControl === "restart" ? "Restarting..." : "Restart T1/T2/T3"}
                </Button>
              </div>

              {taskControlError ? (
                <p className="text-destructive text-xs whitespace-pre-wrap">
                  {taskControlError}
                </p>
              ) : null}

              <TaskBudgetPanel
                onChanged={async () => {
                  await loadTasks();
                }}
                task={selectedTask}
                title="Task Token Budget"
              />

              <div className="flex flex-wrap gap-2">
                <Button
                  onClick={() => {
                    selectTask(selectedTask.id);
                    goToTab("mutations");
                  }}
                  size="sm"
                  type="button"
                >
                  Open In Mutations
                </Button>
                <Button
                  onClick={() => {
                    selectTask(selectedTask.id);
                    goToTab("dashboard");
                  }}
                  size="sm"
                  type="button"
                  variant="outline"
                >
                  Open In Dashboard
                </Button>
              </div>

              <TaskActivityFeed
                taskId={selectedTask.id}
                title="Orchestrator + Agents Activity"
              />
            </>
          ) : (
            <p className="text-muted-foreground text-sm">
              Select a task in the graph to inspect details.
            </p>
          )}
        </CardContent>
      </Card>
      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle>Task Hierarchy</CardTitle>
          <div className="flex gap-2">
            <Button
              onClick={() => void loadTasks()}
              size="sm"
              type="button"
              variant="outline"
            >
              {isLoadingTasks ? "Refreshing..." : "Refresh"}
            </Button>
            <Button
              onClick={() => setIsCreateDialogOpen(true)}
              size="sm"
              type="button"
            >
              <Plus className="mr-2 h-4 w-4" />
              New Task
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          {taskLoadError ? (
            <p className="text-destructive mb-3 text-sm">{taskLoadError}</p>
          ) : null}
          <TaskGraph
            tasks={tasks}
            selectedTaskId={selectedTaskId}
            onTaskClick={(taskId) => {
              selectTask(taskId);
            }}
            onTaskDoubleClick={(taskId) => {
              selectTask(taskId);
              goToTab("mutations");
            }}
          />
        </CardContent>
      </Card>

      <Dialog
        open={isCreateDialogOpen}
        onOpenChange={(open) => {
          setIsCreateDialogOpen(open);
          if (!open) {
            resetCreateTaskState();
          }
        }}
      >
        <DialogContent>
          <form
            className="flex w-full flex-col space-y-4"
            onSubmit={handleCreateTask}
          >
            <DialogHeader>
              <DialogTitle>Create Task</DialogTitle>
              <DialogDescription>
                Create a new orchestrator, domain, or specialist task.
              </DialogDescription>
            </DialogHeader>

            <div className="space-y-2">
              <Label htmlFor="task-tier">Tier</Label>
              <Select
                onValueChange={(value) =>
                  setTaskForm((previous) => ({
                    ...previous,
                    tier: Number(value) as 1 | 2 | 3,
                  }))
                }
                value={String(taskForm.tier)}
              >
                <SelectTrigger className="w-full" id="task-tier">
                  <SelectValue placeholder="Select task tier" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="1">1 (Orchestrator)</SelectItem>
                  <SelectItem value="2">2 (Domain Leader)</SelectItem>
                  <SelectItem value="3">3 (Specialist)</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-2">
              <Label htmlFor="task-domain">Domain</Label>
              <Input
                id="task-domain"
                onChange={(event) =>
                  setTaskForm((previous) => ({
                    ...previous,
                    domain: event.target.value,
                  }))
                }
                placeholder="auth, ui, infra..."
                value={taskForm.domain}
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="task-token-budget">Token Budget</Label>
              <Input
                id="task-token-budget"
                min={1}
                onChange={(event) =>
                  setTaskForm((previous) => ({
                    ...previous,
                    tokenBudget: Number(event.target.value || 0),
                  }))
                }
                type="number"
                value={taskForm.tokenBudget}
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="task-objective">Objective</Label>
              <Textarea
                id="task-objective"
                onChange={(event) =>
                  setTaskForm((previous) => ({
                    ...previous,
                    objective: event.target.value,
                  }))
                }
                placeholder="Describe the expected output and constraints."
                value={taskForm.objective}
              />
            </div>

            {createTaskError ? (
              <p className="text-destructive text-sm">{createTaskError}</p>
            ) : null}

            <DialogFooter>
              <Button
                onClick={() => setIsCreateDialogOpen(false)}
                type="button"
                variant="outline"
              >
                Cancel
              </Button>
              <Button disabled={isCreatingTask} type="submit">
                {isCreatingTask ? "Creating..." : "Create Task"}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  );
}
