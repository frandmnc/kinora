import { useState } from 'react'
import { NavLink, useParams, useSearchParams } from 'react-router'
import { ChevronLeft, ChevronRight, RefreshCw, Table2 } from 'lucide-react'
import { useApi } from '../../shared/useApi'
import type { TableSummary } from '../../shared/types/TableSummary'
import type { TablePage } from '../../shared/types/TablePage'
import type { JsonValue } from '../../shared/types/serde_json/JsonValue'
import { Alert, Badge, Card, EmptyState, PageHeader } from '../ui'

const PAGE_SIZE = 100

/** Depuración: lista las tablas de SQLite y muestra las filas de la elegida. Solo lectura. */
export function DatabasePage() {
  const { table } = useParams()
  const { data: tables, error, reload } = useApi<TableSummary[]>('/api/admin/debug/tables')

  return (
    <>
      <PageHeader
        title="Base de datos"
        description="Las tablas de SQLite tal como están, de solo lectura. Para depurar."
        actions={
          <button className="btn" onClick={reload}>
            <RefreshCw /> Actualizar
          </button>
        }
      />
      {error && <Alert>{error}</Alert>}
      <div className="db-layout">
        <Card title="Tablas">
          <ul className="db-tables">
            {tables?.map((t) => (
              <li key={t.name}>
                <NavLink to={`/database/${t.name}`}>
                  <span className="mono">{t.name}</span>
                  <Badge>{t.rows}</Badge>
                </NavLink>
              </li>
            ))}
          </ul>
        </Card>
        {table ? (
          <TableView key={table} table={table} />
        ) : (
          <Card>
            <EmptyState icon={Table2} title="Elegí una tabla" />
          </Card>
        )}
      </div>
    </>
  )
}

function TableView({ table }: { table: string }) {
  const [params, setParams] = useSearchParams()
  const offset = Number(params.get('offset') ?? 0)
  const { data: page, error, loading } = useApi<TablePage>(
    `/api/admin/debug/tables/${encodeURIComponent(table)}?limit=${PAGE_SIZE}&offset=${offset}`,
  )

  if (error) return <Alert>{error}</Alert>
  if (!page) return loading ? <p className="muted">Cargando…</p> : null

  const last = Math.min(offset + page.rows.length, page.total)
  const goTo = (newOffset: number) => setParams(newOffset > 0 ? { offset: String(newOffset) } : {})

  return (
    <Card
      title={<span className="mono">{table}</span>}
      description={page.total === 0 ? 'sin filas' : `filas ${offset + 1}–${last} de ${page.total}`}
      actions={
        <>
          <button
            className="btn btn-sm btn-icon"
            title="Anterior"
            disabled={offset === 0}
            onClick={() => goTo(Math.max(0, offset - PAGE_SIZE))}
          >
            <ChevronLeft />
          </button>
          <button
            className="btn btn-sm btn-icon"
            title="Siguiente"
            disabled={last >= page.total}
            onClick={() => goTo(offset + PAGE_SIZE)}
          >
            <ChevronRight />
          </button>
        </>
      }
    >
      <div className="table-scroll">
        <table className="table compact data">
          <thead>
            <tr>
              {page.columns.map((column) => (
                <th key={column.name}>
                  {column.name} <span className="sql-type">{column.sql_type}</span>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {page.rows.map((row, i) => (
              <tr key={i}>
                {row.map((value, j) => (
                  <Cell key={j} value={value} />
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Card>
  )
}

/** Muestra un valor. Los textos largos (como el JSON de ffprobe) se expanden con un clic. */
function Cell({ value }: { value: JsonValue }) {
  const [expanded, setExpanded] = useState(false)

  if (value === null) return <td className="null">NULL</td>
  const text = String(value)
  if (text.length <= 80) return <td className="mono">{text}</td>

  return (
    <td className="mono long" onClick={() => setExpanded(!expanded)} title="Clic para expandir">
      {expanded ? <pre>{prettyIfJson(text)}</pre> : `${text.slice(0, 80)}…`}
    </td>
  )
}

function prettyIfJson(text: string) {
  try {
    return JSON.stringify(JSON.parse(text), null, 2)
  } catch {
    return text
  }
}
