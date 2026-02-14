import { useMemo } from 'react'

import {
  CartesianGrid,
  Legend,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'

import { formatTokenCount } from '@/lib/format'
import type { TaskRecord } from '@/types'

interface TokenBurnChartProps {
  tasks: TaskRecord[]
}

interface ChartPoint {
  compliance: number
  index: number
  label: string
  tokens: number
  formattedTokens: string
}

function buildSeries(tasks: TaskRecord[]): ChartPoint[] {
  const sorted = [...tasks].sort((left, right) => left.createdAt - right.createdAt)
  let cumulativeTokens = 0
  let cumulativeCompliance = 0

  return sorted.map((task, index) => {
    cumulativeTokens += task.tokenUsage
    cumulativeCompliance += task.complianceScore
    return {
      index: index + 1,
      label: `T${task.tier}-${task.id.slice(0, 4)}`,
      tokens: cumulativeTokens,
      formattedTokens: formatTokenCount(cumulativeTokens),
      compliance: cumulativeCompliance,
    }
  })
}

function detectLowEfficiency(series: ChartPoint[]): boolean {
  if (series.length < 4) {
    return false
  }

  let stagnantTicks = 0
  for (let idx = 1; idx < series.length; idx += 1) {
    const tokenDelta = series[idx].tokens - series[idx - 1].tokens
    const complianceDelta = series[idx].compliance - series[idx - 1].compliance
    if (tokenDelta > 1000 && complianceDelta === 0) {
      stagnantTicks += 1
      if (stagnantTicks >= 3) return true
      continue
    }
    stagnantTicks = 0
  }

  return false
}

function TokenBurnChart({ tasks }: TokenBurnChartProps) {
  const series = useMemo(() => buildSeries(tasks), [tasks])
  const lowEfficiency = useMemo(() => detectLowEfficiency(series), [series])

  if (series.length === 0) {
    return <p className="empty-state">No task metrics available for token burn chart.</p>
  }

  return (
    <div className="token-chart-shell">
      {lowEfficiency ? <p className="feedback">Low efficiency detected: token burn increased with no compliance gains.</p> : null}
      <ResponsiveContainer height={280} width="100%">
        <LineChart data={series}>
          <CartesianGrid strokeDasharray="3 3" />
          <XAxis dataKey="label" minTickGap={20} />
          <YAxis yAxisId="left" />
          <YAxis orientation="right" yAxisId="right" />
          <Tooltip formatter={(value, name) => name === 'tokens' ? formatTokenCount(value as number) : value} />
          <Legend />
          <Line
            dataKey="tokens"
            name="Cumulative Tokens"
            stroke="#2563eb"
            strokeWidth={2}
            type="monotone"
            yAxisId="left"
          />
          <Line
            dataKey="compliance"
            name="Cumulative Compliance"
            stroke="#16a34a"
            strokeWidth={2}
            type="monotone"
            yAxisId="right"
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  )
}

export default TokenBurnChart