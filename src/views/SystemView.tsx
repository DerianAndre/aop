import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'

export function SystemView() {
  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle>Component Health</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="w-2 h-2 bg-green-500 rounded-full" />
              <div>
                <p className="text-sm font-medium">MCP Bridge</p>
                <p className="text-xs text-muted-foreground">Uptime: 2h 34m</p>
              </div>
            </div>
            <Badge variant="secondary">OK</Badge>
          </div>

          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="w-2 h-2 bg-green-500 rounded-full" />
              <div>
                <p className="text-sm font-medium">Vector Engine</p>
                <p className="text-xs text-muted-foreground">15,234 chunks indexed</p>
              </div>
            </div>
            <Badge variant="secondary">OK</Badge>
          </div>

          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="w-2 h-2 bg-green-500 rounded-full" />
              <div>
                <p className="text-sm font-medium">SQLite Database</p>
                <p className="text-xs text-muted-foreground">34.2 MB</p>
              </div>
            </div>
            <Badge variant="secondary">OK</Badge>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
