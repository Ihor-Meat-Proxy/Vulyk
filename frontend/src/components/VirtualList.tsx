import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react'

type Props<T> = {
  items: T[]
  rowHeight: number
  height: number
  overscan?: number
  renderRow: (item: T, index: number) => ReactNode
  className?: string
}

/** Lightweight windowing for large case lists (100k+ friendly when API paginates). */
export function VirtualList<T>({
  items,
  rowHeight,
  height,
  overscan = 8,
  renderRow,
  className,
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

  return (
    <div
      ref={ref}
      className={className}
      style={{ height, overflow: 'auto', position: 'relative' }}
    >
      <div style={{ height: items.length * rowHeight, position: 'relative' }}>
        <div style={{ transform: `translateY(${offsetY}px)` }}>
          {slice.map((item, i) => (
            <div key={start + i} style={{ height: rowHeight }}>
              {renderRow(item, start + i)}
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
