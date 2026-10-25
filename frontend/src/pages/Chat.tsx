import { FormEvent, useState } from 'react'
import { api } from '../api'
import { useProjectPermissions } from '../hooks/useProjectPermissions'

type Props = {
  projectId: string | null
}

type Turn = { role: 'user' | 'assistant'; text: string; meta?: string }

export function Chat({ projectId }: Props) {
  const { can } = useProjectPermissions(projectId)
  const canLlm = can('llm_assist')

  const [question, setQuestion] = useState('')
  const [turns, setTurns] = useState<Turn[]>([])
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const onAsk = async (e: FormEvent) => {
    e.preventDefault()
    if (!projectId || !question.trim()) return
    const q = question.trim()
    setQuestion('')
    setTurns((t) => [...t, { role: 'user', text: q }])
    setBusy(true)
    setError(null)
    try {
      const res = await api.askTests(projectId, q)
      setTurns((t) => [
        ...t,
        {
          role: 'assistant',
          text: res.answer,
          meta: `${res.provider} / ${res.model}`,
        },
      ])
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  if (!projectId) {
    return <p className="muted">Select a project to chat with its tests.</p>
  }

  if (!canLlm) {
    return (
      <div className="panel">
        <p className="muted">You do not have permission to use LLM assist.</p>
      </div>
    )
  }

  return (
    <div className="chat-page">
      <header className="panel-header">
        <h2>Chat with your tests</h2>
        <p className="muted">
          Ask about coverage, gaps, or case intent. Uses the project LLM config
          (mock when none is set).
        </p>
      </header>

      <div className="chat-log" aria-live="polite">
        {turns.length === 0 && (
          <p className="muted">Try “Which cases cover login failures?”</p>
        )}
        {turns.map((turn, i) => (
          <div key={i} className={`chat-bubble ${turn.role}`}>
            <div className="chat-role">{turn.role === 'user' ? 'You' : 'Vulyk'}</div>
            <div className="chat-text">{turn.text}</div>
            {turn.meta && <div className="muted chat-meta">{turn.meta}</div>}
          </div>
        ))}
      </div>

      {error && <p className="error">{error}</p>}

      <form className="chat-compose" onSubmit={onAsk}>
        <input
          value={question}
          onChange={(e) => setQuestion(e.target.value)}
          placeholder="Ask about this project’s tests…"
          disabled={busy}
        />
        <button type="submit" disabled={busy || !question.trim()}>
          {busy ? 'Thinking…' : 'Ask'}
        </button>
      </form>
    </div>
  )
}
