/**
 * A virtualized list: only the visible rows exist in the DOM, and rows are
 * fetched in pages as they scroll into view, so lists with more than
 * 100,000 entries stay fast. Give it a new `key` for a new search.
 */
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from 'react';

/** Rows per page request. */
export const PAGE_SIZE = 200;
/** Rows drawn above and below the visible ones. */
const OVERSCAN = 8;

export interface VirtualListProps<T> {
  /** Number of rows in total. */
  readonly total: number;
  /** Fixed row height in CSS pixels. */
  readonly rowHeight: number;
  /** Loads `limit` rows starting at `offset`. */
  readonly fetchPage: (offset: number, limit: number) => Promise<readonly T[]>;
  readonly renderRow: (item: T, index: number, selected: boolean) => ReactNode;
  readonly rowKey: (item: T) => string | number;
  readonly selected: string | number | null;
  readonly onSelect: (item: T, index: number) => void;
  readonly label: string;
  readonly testId: string;
  readonly empty?: ReactNode;
}

export function VirtualList<T>({
  total,
  rowHeight,
  fetchPage,
  renderRow,
  rowKey,
  selected,
  onSelect,
  label,
  testId,
  empty,
}: VirtualListProps<T>): ReactNode {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [height, setHeight] = useState(400);
  const [rows, setRows] = useState<ReadonlyMap<number, T>>(() => new Map());
  const pagesRef = useRef(new Set<number>());
  const [focusIndex, setFocusIndex] = useState(0);

  useEffect(() => {
    const element = scrollRef.current;
    if (element === null) {
      return undefined;
    }
    const observer = new ResizeObserver(() => {
      setHeight(element.clientHeight);
    });
    observer.observe(element);
    return () => {
      observer.disconnect();
    };
  }, []);

  const first = Math.max(0, Math.floor(scrollTop / rowHeight) - OVERSCAN);
  const last = Math.min(total, Math.ceil((scrollTop + height) / rowHeight) + OVERSCAN);

  // Fetch the pages of the visible rows.
  useEffect(() => {
    let cancelled = false;
    const wanted = new Set<number>();
    for (let index = first; index < last; index += PAGE_SIZE) {
      wanted.add(Math.floor(index / PAGE_SIZE));
    }
    if (last > first) {
      wanted.add(Math.floor((last - 1) / PAGE_SIZE));
    }
    for (const page of wanted) {
      if (pagesRef.current.has(page)) {
        continue;
      }
      pagesRef.current.add(page);
      const offset = page * PAGE_SIZE;
      void fetchPage(offset, PAGE_SIZE).then(
        (items) => {
          if (cancelled) {
            return;
          }
          setRows((current) => {
            const next = new Map(current);
            items.forEach((item, i) => next.set(offset + i, item));
            return next;
          });
        },
        () => {
          pagesRef.current.delete(page);
        },
      );
    }
    return () => {
      cancelled = true;
    };
  }, [first, last, fetchPage]);

  const scrollIntoView = useCallback(
    (index: number) => {
      const element = scrollRef.current;
      if (element === null) {
        return;
      }
      const top = index * rowHeight;
      if (top < element.scrollTop) {
        element.scrollTop = top;
      } else if (top + rowHeight > element.scrollTop + element.clientHeight) {
        element.scrollTop = top + rowHeight - element.clientHeight;
      }
    },
    [rowHeight],
  );

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    const page = Math.max(1, Math.floor(height / rowHeight) - 1);
    const moves: Record<string, number> = {
      ArrowDown: 1,
      ArrowUp: -1,
      PageDown: page,
      PageUp: -page,
      Home: -total,
      End: total,
    };
    const move = moves[event.key];
    if (move === undefined || total === 0) {
      return;
    }
    event.preventDefault();
    const index = Math.min(total - 1, Math.max(0, focusIndex + move));
    setFocusIndex(index);
    scrollIntoView(index);
    const item = rows.get(index);
    if (item !== undefined) {
      onSelect(item, index);
    }
  };

  const visible: ReactNode[] = [];
  for (let index = first; index < last; index++) {
    const item = rows.get(index);
    const isSelected = item !== undefined && rowKey(item) === selected;
    visible.push(
      <div
        key={item === undefined ? `placeholder-${index.toString()}` : rowKey(item)}
        className={`vlist__row${isSelected ? ' vlist__row--selected' : ''}`}
        style={{ top: index * rowHeight, height: rowHeight }}
        role="option"
        aria-selected={isSelected}
        aria-posinset={index + 1}
        aria-setsize={total}
        data-index={index}
        onClick={() => {
          if (item !== undefined) {
            setFocusIndex(index);
            onSelect(item, index);
          }
        }}
      >
        {item === undefined ? (
          <span className="vlist__placeholder" />
        ) : (
          renderRow(item, index, isSelected)
        )}
      </div>,
    );
  }

  return (
    <div
      ref={scrollRef}
      className="vlist"
      role="listbox"
      aria-label={label}
      tabIndex={0}
      data-testid={testId}
      data-total={total}
      onScroll={(event) => {
        setScrollTop(event.currentTarget.scrollTop);
      }}
      onKeyDown={onKeyDown}
    >
      {total === 0 && empty !== undefined ? (
        <div className="vlist__empty">{empty}</div>
      ) : (
        <div className="vlist__content" style={{ height: total * rowHeight }}>
          {visible}
        </div>
      )}
    </div>
  );
}
