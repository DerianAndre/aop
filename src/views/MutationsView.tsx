import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import DiffReviewer from '@/components/DiffReviewer'

export function MutationsView() {
  return (
    <div className="grid grid-cols-1 lg:grid-cols-[300px_1fr] gap-4">
      {/* Left: Approval Queue */}
      <Card>
        <CardHeader>
          <CardTitle>Approval Queue</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">No pending mutations</p>
        </CardContent>
      </Card>

      {/* Right: Diff Reviewer */}
      <Card>
        <CardHeader>
          <CardTitle>Diff Reviewer</CardTitle>
        </CardHeader>
        <CardContent>
          <DiffReviewer
            isBusy={false}
            mutation={null}
            originalContent=""
            onApprove={async () => {}}
            onReject={async () => {}}
            onRequestRevision={async () => {}}
          />
        </CardContent>
      </Card>
    </div>
  )
}
