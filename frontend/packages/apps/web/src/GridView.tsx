import { useVirtualizer } from '@tanstack/react-virtual';
import { useEffect, useRef, useState } from 'react';

import type { Entry } from './layout';
import { columnCount, rowCount, rowItems } from './layout';

const CELL_WIDTH = 220;
const CELL_HEIGHT = 180;

export function GridView({
  entries,
  onOpen,
}: {
  entries: Entry[];
  onOpen: (index: number) => void;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);

  useEffect(() => {
    const element = scrollRef.current;
    if (!element) {
      return;
    }
    const observer = new ResizeObserver(() => setWidth(element.clientWidth));
    observer.observe(element);
    setWidth(element.clientWidth);
    return () => observer.disconnect();
  }, []);

  const columns = columnCount(width, CELL_WIDTH);
  const rows = rowCount(entries.length, columns);
  const virtualizer = useVirtualizer({
    count: rows,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => CELL_HEIGHT,
    overscan: 4,
  });

  return (
    <div ref={scrollRef} style={{ flex: 1, overflowY: 'auto' }}>
      <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
        {virtualizer.getVirtualItems().map((row) => (
          <div
            key={row.key}
            style={{
              position: 'absolute',
              top: 0,
              left: 0,
              width: '100%',
              height: CELL_HEIGHT,
              transform: `translateY(${row.start}px)`,
              display: 'flex',
            }}
          >
            {rowItems(row.index, columns, entries.length).map((index) => (
              <button
                key={index}
                onClick={() => onOpen(index)}
                title={entries[index].path}
                style={{
                  flex: `0 0 ${100 / columns}%`,
                  padding: 4,
                  border: 'none',
                  background: 'none',
                  cursor: 'pointer',
                }}
              >
                <img
                  src={`/api/thumb/${entries[index].id}`}
                  loading="lazy"
                  style={{
                    width: '100%',
                    height: '100%',
                    objectFit: 'cover',
                    borderRadius: 4,
                    background: '#222',
                  }}
                />
              </button>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}
