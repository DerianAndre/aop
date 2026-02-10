import { create } from 'zustand'
import { devtools, persist } from 'zustand/middleware'
import type { AopStoreState, AopStoreActions } from './types'
import type { TaskStatus } from '@/types'

type AopStore = AopStoreState & AopStoreActions

export const useAopStore = create<AopStore>()(
  devtools(
    persist(
      (set, get) => ({
        // Initial state
        tasks: new Map(),
        mutations: new Map(),
        contextQueries: [],
        selectedTaskId: null,
        selectedMutationId: null,
        activeTab: 'tasks',
        taskFilter: {},
        indexStatus: null,
        sidecarHealth: null,
        targetProject: '',
        mcpCommand: '',
        mcpArgs: '',

        // Actions
        addTask: (task) =>
          set((state) => ({
            tasks: new Map(state.tasks).set(task.id, task),
          })),

        updateTask: (taskId, updates) =>
          set((state) => {
            const task = state.tasks.get(taskId)
            if (!task) return state
            return {
              tasks: new Map(state.tasks).set(taskId, { ...task, ...updates }),
            }
          }),

        addMutation: (mutation) =>
          set((state) => ({
            mutations: new Map(state.mutations).set(mutation.id, mutation),
          })),

        updateMutation: (mutationId, updates) =>
          set((state) => {
            const mutation = state.mutations.get(mutationId)
            if (!mutation) return state
            return {
              mutations: new Map(state.mutations).set(mutationId, {
                ...mutation,
                ...updates,
              }),
            }
          }),

        selectTask: (taskId) => set({ selectedTaskId: taskId }),
        selectMutation: (mutationId) => set({ selectedMutationId: mutationId }),
        setActiveTab: (tab) => set({ activeTab: tab }),
        setTaskFilter: (filter) =>
          set((state) => ({
            taskFilter: { ...state.taskFilter, ...filter },
          })),

        setIndexStatus: (status) => set({ indexStatus: status }),
        setSidecarHealth: (health) => set({ sidecarHealth: health }),
        setTargetProject: (value) => set({ targetProject: value }),
        setMcpCommand: (value) => set({ mcpCommand: value }),
        setMcpArgs: (value) => set({ mcpArgs: value }),

        handleTauriEvent: (event) => {
          const { addTask, updateTask, addMutation, updateMutation } = get()

          switch (event.type) {
            case 'task_created':
              addTask(event.task)
              break
            case 'task_status_changed':
              updateTask(event.task_id, { status: event.new_status as TaskStatus })
              break
            case 'mutation_proposed':
              addMutation(event.mutation)
              break
            case 'mutation_status_changed':
              updateMutation(event.mutation_id, { status: event.new_status })
              break
            case 'token_usage':
              updateTask(event.task_id, { tokenUsage: event.tokens_spent })
              break
            case 'context_query':
              set((state) => ({
                contextQueries: [event.query, ...state.contextQueries].slice(0, 20),
              }))
              break
          }
        },
      }),
      {
        name: 'aop-storage',
        partialize: (state) => ({
          // Only persist user preferences
          activeTab: state.activeTab,
          taskFilter: state.taskFilter,
          targetProject: state.targetProject,
          mcpCommand: state.mcpCommand,
          mcpArgs: state.mcpArgs,
        }),
      }
    )
  )
)
