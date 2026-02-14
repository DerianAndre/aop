// AOP(security_analyst): Implement core changes: Add a TypeScript utility function that formats token counts with K/M suffixes (e.g., 1500 → "1.5
import { useEffect } from 'react'
import { AppLayout } from '@/layouts/AppLayout'
import { useAopStore } from '@/store/aop-store'
import { Toaster } from 'sonner'

function App() {
  const handleTauriEvent = useAopStore((state) => state.handleTauriEvent)

  useEffect(() => {
    // TODO: Set up Tauri event listeners for real-time updates
    // Example:
    // import { listen } from '@tauri-apps/api/event'
    // const unlisten = await listen('aop-event', (event) => {
    //   handleTauriEvent(event.payload)
    // })
    // return () => { unlisten() }
  }, [handleTauriEvent])

  return (
    <>
      <AppLayout />
      <Toaster position="top-right" richColors />
    </>
  )
}

export default App
