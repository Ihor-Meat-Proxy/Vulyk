type Props = {
  title: string
  blurb: string
}

export function Placeholder({ title, blurb }: Props) {
  return (
    <div className="panel">
      <h2>{title}</h2>
      <p className="muted">{blurb}</p>
    </div>
  )
}
