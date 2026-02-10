import type { ConflictReport, DiffProposal, MutationRecord } from '@/types'

interface ConflictResolutionPanelProps {
  conflict: ConflictReport | undefined
  mutations: MutationRecord[]
  proposals: DiffProposal[]
  onAcceptProposal: (agentUid: string) => Promise<void>
  onMergeManually: () => void
  onRejectBoth: (agentUidA: string, agentUidB: string) => Promise<void>
}

function proposalToMutation(agentUid: string, mutations: MutationRecord[]): MutationRecord | undefined {
  return mutations.find((mutation) => mutation.agentUid === agentUid)
}

function ConflictResolutionPanel({
  conflict,
  proposals,
  mutations,
  onAcceptProposal,
  onRejectBoth,
  onMergeManually,
}: ConflictResolutionPanelProps) {
  if (!conflict || proposals.length < 2) {
    return null
  }

  const proposalA = proposals.find((proposal) => proposal.agentUid === conflict.agentA) ?? proposals[0]
  const proposalB = proposals.find((proposal) => proposal.agentUid === conflict.agentB) ?? proposals[1]
  const mutationA = proposalToMutation(proposalA.agentUid, mutations)
  const mutationB = proposalToMutation(proposalB.agentUid, mutations)

  return (
    <section className="card browser-card">
      <div className="card-header">
        <h2 className="card-title">Conflict Resolution</h2>
      </div>
      <div className="card-content">
        <p className="feedback">
          Consensus failed. Semantic distance {conflict.semanticDistance.toFixed(3)} between proposals.
        </p>
        <div className="diff-grid">
          <div className="browser-panel">
            <div className="browser-panel-header">
              <strong>Proposal A</strong>
              <span className="meta-inline">{proposalA.agentUid.slice(0, 8)}</span>
            </div>
            <p className="task-objective">{proposalA.intentDescription}</p>
            <pre className="file-preview">{proposalA.diffContent}</pre>
          </div>
          <div className="browser-panel">
            <div className="browser-panel-header">
              <strong>Proposal B</strong>
              <span className="meta-inline">{proposalB.agentUid.slice(0, 8)}</span>
            </div>
            <p className="task-objective">{proposalB.intentDescription}</p>
            <pre className="file-preview">{proposalB.diffContent}</pre>
          </div>
        </div>
        <div className="mutation-actions">
          <button className="tier2-run-button" type="button" onClick={() => void onAcceptProposal(proposalA.agentUid)}>
            Accept A
          </button>
          <button className="tier2-run-button" type="button" onClick={() => void onAcceptProposal(proposalB.agentUid)}>
            Accept B
          </button>
          <button
            className="tier2-run-button"
            type="button"
            onClick={() => void onRejectBoth(proposalA.agentUid, proposalB.agentUid)}
          >
            Reject Both
          </button>
          <button className="tier2-run-button" type="button" onClick={onMergeManually}>
            Merge Manually
          </button>
        </div>
        <p className="meta-inline">
          Linked mutations: {mutationA ? mutationA.id.slice(0, 8) : 'n/a'} | {mutationB ? mutationB.id.slice(0, 8) : 'n/a'}
        </p>
      </div>
    </section>
  )
}

export default ConflictResolutionPanel
