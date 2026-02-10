import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'

export function ContextView() {
  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle>Vector Index Status</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <span className="text-sm text-muted-foreground">Indexed Files</span>
              <Badge variant="secondary">2,847</Badge>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm text-muted-foreground">Chunks</span>
              <Badge variant="secondary">15,234</Badge>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm text-muted-foreground">Last Indexed</span>
              <Badge variant="secondary">2 min ago</Badge>
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Live Agent Queries</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">No active queries</p>
        </CardContent>
      </Card>
    </div>
  )
}
