import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from 'react'

type Props<T> = {
  items: T[]
  rowHeight: number
  height: number
  overscan?: number
  renderRow: (item: T, index: number) => ReactNode
  className?: string
  /** Sticky column header rendered above the windowed rows. */
  header?: ReactNode
  /** Optional test id for the scrollport (sticky header stays inside). */
  scrollTestId?: string
}

/** Lightweight windowing for large case lists (no extra dependency). */
export function VirtualList<T>({
  items,
  rowHeight,
  height,
  overscan = 8,
  renderRow,
  className,
  header,
  scrollTestId,
}: Props<T>) {
  const ref = useRef<HTMLDivElement>(null)
  const [scrollTop, setScrollTop] = useState(0)

  useEffect(() => {
    const el = ref.current
    if (!el) return
    const onScroll = () => setScrollTop(el.scrollTop)
    el.addEventListener('scroll', onScroll, { passive: true })
    return () => el.removeEventListener('scroll', onScroll)
  }, [])

  const { start, end, offsetY } = useMemo(() => {
    const startIdx = Math.max(0, Math.floor(scrollTop / rowHeight) - overscan)
    const visible = Math.ceil(height / rowHeight) + overscan * 2
    const endIdx = Math.min(items.length, startIdx + visible)
    return {
      start: startIdx,
      end: endIdx,
      offsetY: startIdx * rowHeight,
    }
  }, [scrollTop, rowHeight, height, overscan, items.length])

  const slice = items.slice(start, end)

  const rowStyle: CSSProperties = {
    height: rowHeight,
    contentVisibility: 'auto',
    containIntrinsicSize: `auto ${rowHeight}px`,
  }

  return (
    <div
      ref={ref}
      className={className}
      data-testid={scrollTestId}
      style={{ height, overflow: 'auto', position: 'relative' }}
    >
      {header ? (
        <div className="virtual-list-sticky-header">{header}</div>
      ) : null}
      <div style={{ height: items.length * rowHeight, position: 'relative' }}>
        <div style={{ transform: `translateY(${offsetY}px)` }}>
          {slice.map((item, i) => (
            <div key={start + i} style={rowStyle}>
              {renderRow(item, start + i)}
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
