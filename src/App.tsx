import { useEffect } from 'react'
import { AppLayout } from '@/layouts/AppLayout'
import { useAopStore } from '@/store/aop-store'

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

  return <AppLayout />
}

export default App
