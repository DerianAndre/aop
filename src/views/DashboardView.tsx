import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import TokenBurnChart from '@/components/TokenBurnChart'

export function DashboardView() {
  return (
    <div className="space-y-6">
      {/* Metric Cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <Card>
          <CardHeader>
            <CardTitle className="text-sm font-medium">Active Tasks</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold">15</div>
            <p className="text-xs text-muted-foreground">3 executing</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="text-sm font-medium">Tokens Spent</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold">12.4K</div>
            <p className="text-xs text-muted-foreground">of 50K budget</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="text-sm font-medium">System Health</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-3xl font-bold">94.3%</div>
            <p className="text-xs text-muted-foreground">All systems operational</p>
          </CardContent>
        </Card>
      </div>

      {/* Token Burn Chart */}
      <Card>
        <CardHeader>
          <CardTitle>Token Burn Over Time</CardTitle>
        </CardHeader>
        <CardContent>
          <TokenBurnChart tasks={[]} />
        </CardContent>
      </Card>
    </div>
  )
}
