import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'

export function LogsView() {
  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between">
        <CardTitle>System Logs</CardTitle>
        <div className="flex gap-2">
          <Badge variant="outline">System</Badge>
          <Badge variant="outline">MCP</Badge>
          <Badge variant="outline">Agents</Badge>
        </div>
      </CardHeader>
      <CardContent>
        <ScrollArea className="h-[600px]">
          <div className="space-y-2 font-mono text-xs">
            <p className="text-muted-foreground">No logs yet</p>
          </div>
        </ScrollArea>
      </CardContent>
    </Card>
  )
}
