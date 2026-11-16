import {
  useCallback,
  useRef,
  useState,
  type ChangeEvent,
  type DragEvent,
  type ReactNode,
  type Ref,
} from 'react'
import { testIdProps } from '../testing'

export type FileDropZoneProps = {
  accept?: string
  disabled?: boolean
  multiple?: boolean
  capture?: boolean | 'user' | 'environment'
  onFiles: (files: File[]) => void
  className?: string
  'aria-label': string
  dropZoneTestId: string
  inputTestId?: string
  inputRef?: Ref<HTMLInputElement>
  children?: ReactNode
}

export function FileDropZone({
  accept,
  disabled = false,
  multiple = false,
  capture,
  onFiles,
  className = '',
  'aria-label': ariaLabel,
  dropZoneTestId,
  inputTestId,
  inputRef,
  children,
}: FileDropZoneProps) {
  const localRef = useRef<HTMLInputElement | null>(null)
  const [dragging, setDragging] = useState(false)

  const assignRef = useCallback(
    (node: HTMLInputElement | null) => {
      localRef.current = node
      if (typeof inputRef === 'function') {
        inputRef(node)
      } else if (inputRef && 'current' in inputRef) {
        ;(inputRef as { current: HTMLInputElement | null }).current = node
      }
    },
    [inputRef],
  )

  const emitFiles = useCallback(
    (list: FileList | File[] | null | undefined) => {
      if (disabled || !list) return
      const files = Array.from(list)
      if (files.length === 0) return
      onFiles(multiple ? files : files.slice(0, 1))
    },
    [disabled, multiple, onFiles],
  )

  const onDragOver = (e: DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    if (disabled) return
    setDragging(true)
  }

  const onDragLeave = (e: DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    setDragging(false)
  }

  const onDrop = (e: DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    setDragging(false)
    if (disabled) return
    emitFiles(e.dataTransfer.files)
  }

  const onChange = (e: ChangeEvent<HTMLInputElement>) => {
    emitFiles(e.target.files)
    e.target.value = ''
  }

  const classes = [
    'file-drop-zone',
    dragging ? 'is-dragover' : '',
    disabled ? 'is-disabled' : '',
    className,
  ]
    .filter(Boolean)
    .join(' ')

  return (
    // Label associates the visible control with the hidden file input (OS picker).
    // Drag handlers are intentional; nested buttons stopPropagation and open the input themselves.
    // eslint-disable-next-line jsx-a11y/no-noninteractive-element-interactions -- drop target
    <label
      className={classes}
      aria-label={ariaLabel}
      aria-disabled={disabled || undefined}
      onDragOver={onDragOver}
      onDragEnter={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
      {...testIdProps(dropZoneTestId)}
    >
      {children}
      <input
        ref={assignRef}
        type="file"
        className="file-input"
        accept={accept}
        multiple={multiple}
        capture={capture}
        disabled={disabled}
        aria-label={ariaLabel}
        onChange={onChange}
        {...(inputTestId ? testIdProps(inputTestId) : {})}
      />
    </label>
  )
}
