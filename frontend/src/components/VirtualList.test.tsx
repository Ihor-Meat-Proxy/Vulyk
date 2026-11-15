/** @vitest-environment jsdom */
import { act } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { VirtualList } from './VirtualList'

describe('VirtualList', () => {
  let container: HTMLDivElement
  let root: Root

  beforeEach(() => {
    container = document.createElement('div')
    document.body.appendChild(container)
    root = createRoot(container)
  })

  afterEach(() => {
    act(() => {
      root.unmount()
    })
    container.remove()
  })

  it('renders sticky header and only a window of rows', () => {
    const items = Array.from({ length: 100 }, (_, i) => `row-${i}`)
    act(() => {
      root.render(
        <VirtualList
          items={items}
          rowHeight={40}
          height={200}
          overscan={2}
          scrollTestId="case-list-virtual-scroll"
          header={<div id="virt-head">Columns</div>}
          renderRow={(item) => <div className="virt-row">{item}</div>}
        />,
      )
    })

    expect(container.querySelector('#virt-head')).toBeTruthy()
    expect(
      container.querySelector('[data-testid="case-list-virtual-scroll"]'),
    ).toBeTruthy()
    const rows = container.querySelectorAll('.virt-row')
    // 200/40 = 5 visible + overscan*2 ≈ well under 100
    expect(rows.length).toBeLessThan(30)
    expect(rows.length).toBeGreaterThan(0)
    expect(rows[0]?.textContent).toBe('row-0')
  })
})
