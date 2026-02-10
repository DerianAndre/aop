import { useState } from 'react'

import type { MutationRecord } from '@/types'

interface DiffReviewerProps {
  isBusy: boolean
  mutation: MutationRecord | null
  originalContent: string
  onApprove: (mutationId: string) => Promise<void>
  onReject: (mutationId: string, reason: string) => Promise<void>
  onRequestRevision: (mutationId: string, note: string) => Promise<void>
}

function DiffReviewer({
  mutation,
  originalContent,
  onApprove,
  onReject,
  onRequestRevision,
  isBusy,
}: DiffReviewerProps) {
  const [rejectReason, setRejectReason] = useState('Does not preserve intended behavior.')
  const [revisionNote, setRevisionNote] = useState('Please rework and narrow diff scope.')

  if (!mutation) {
    return <p className="empty-state">Select a mutation to open the diff reviewer.</p>
  }

  return (
    <div className="diff-reviewer-shell">
      <div className="task-row">
        <strong>{mutation.filePath}</strong>
        <span className="meta-inline">confidence {mutation.confidence.toFixed(2)}</span>
      </div>
      <p className="task-objective">{mutation.intentDescription ?? 'No intent description provided.'}</p>

      <div className="diff-grid">
        <div className="browser-panel">
          <div className="browser-panel-header">
            <strong>Original</strong>
          </div>
          <pre className="file-preview">{originalContent || 'Unable to load original file content.'}</pre>
        </div>
        <div className="browser-panel">
          <div className="browser-panel-header">
            <strong>Proposed Changes</strong>
          </div>
          <pre className="file-preview">{mutation.diffContent}</pre>
        </div>
      </div>

      <div className="browser-inline-grid">
        <div className="field">
          <label htmlFor="reject-reason">Reject Reason</label>
          <input id="reject-reason" value={rejectReason} onChange={(event) => setRejectReason(event.target.value)} />
        </div>
        <div className="field">
          <label htmlFor="revision-note">Revision Note</label>
          <input id="revision-note" value={revisionNote} onChange={(event) => setRevisionNote(event.target.value)} />
        </div>
      </div>

      <div className="mutation-actions">
        <button className="tier2-run-button" disabled={isBusy} type="button" onClick={() => void onApprove(mutation.id)}>
          {isBusy ? 'Working...' : 'Approve'}
        </button>
        <button
          className="tier2-run-button"
          disabled={isBusy}
          type="button"
          onClick={() => void onReject(mutation.id, rejectReason)}
        >
          {isBusy ? 'Working...' : 'Reject'}
        </button>
        <button
          className="tier2-run-button"
          disabled={isBusy}
          type="button"
          onClick={() => void onRequestRevision(mutation.id, revisionNote)}
        >
          Request Revision
        </button>
      </div>
    </div>
  )
}

export default DiffReviewer
