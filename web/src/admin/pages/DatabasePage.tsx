import { useState } from 'react'
import { NavLink, useParams, useSearchParams } from 'react-router'
import { useApi } from '../../shared/useApi'
import type { TableSummary } from '../../shared/types/TableSummary'
import type { TablePage } from '../../shared/types/TablePage'
import type { JsonValue } from '../../shared/types/serde_json/JsonValue'

const PAGE_SIZE = 100

/** Depuración: lista las tablas de SQLite y muestra las filas de la elegida. Solo lectura. */
export function DatabasePage() {
  const { table } = useParams()
  const { data: tables, error, reload } = useApi<TableSummary[]>('/api/admin/debug/tables')

  return (
    <>
      <div className="page-header">
        <h1>Base de datos</h1>
        <button onClick={reload}>Actualizar</button>
      </div>
      {error && <p className="error">{error}</p>}
      <div className="db-layout">
        <ul className="db-tables">
          {tables?.map((t) => (
            <li key={t.name}>
              <NavLink to={`/database/${t.name}`}>
                <span className="mono">{t.name}</span>
                <span className="count">{t.rows}</span>
              </NavLink>
            </li>
          ))}
        </ul>
        <div className="db-rows">
          {table ? <TableView key={table} table={table} /> : <p className="muted">Elegí una tabla.</p>}
        </div>
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

  if (error) return <p className="error">{error}</p>
  if (!page) return loading ? <p className="muted">Cargando…</p> : null

  const last = Math.min(offset + page.rows.length, page.total)
  const goTo = (newOffset: number) => setParams(newOffset > 0 ? { offset: String(newOffset) } : {})

  return (
    <>
      <div className="pager">
        <span className="muted">
          {page.total === 0 ? 'Sin filas' : `Filas ${offset + 1}–${last} de ${page.total}`}
        </span>
        <button disabled={offset === 0} onClick={() => goTo(Math.max(0, offset - PAGE_SIZE))}>
          ← Anterior
        </button>
        <button disabled={last >= page.total} onClick={() => goTo(offset + PAGE_SIZE)}>
          Siguiente →
        </button>
      </div>
      <div className="table-scroll">
        <table className="table compact">
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
    </>
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
