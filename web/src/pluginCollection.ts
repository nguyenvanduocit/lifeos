// Version 1 of the native, monthly collection view. Assets and tool replies are
// runtime inputs; widgets use the same schema and query as their full page.
export interface Field {
  key: string
  label: string
  type: 'date' | 'text' | 'number' | 'select'
  required?: boolean
  options?: Record<string, string>
  min?: number
  max?: number
  step?: number
  maxlength?: number
  unit?: string
  when?: { key: string; equals: string }
}
export interface Collection {
  key: string
  read: string
  save: string
  remove: string
  period: { parameter: string; field: string; label: string }
  summary: { label: string; amount: string; group: string; currency: string; locale: string; count: string }
  labels: Record<'create'|'edit'|'submit'|'history'|'empty'|'modify'|'delete'|'confirm'|'cancel', string>
  fields: Field[]
}
export type Item = Record<string, string | number | null>

const identifier = (key: unknown) => typeof key === 'string' && /^[a-z][a-z0-9_]*$/.test(key) && !Object.hasOwn(Object.prototype, key)
export function parseCollection(raw: Collection & { version: number; component: string }): Collection {
  const invalid = () => { throw new Error('Invalid plugin collection schema') }
  if (raw?.version !== 1 || raw.component !== 'collection' || !Array.isArray(raw.fields) || !raw.fields.length ||
      ![raw.read, raw.save, raw.remove, raw.key, raw.period?.parameter].every(identifier)) invalid()
  const keys = new Set([raw.key])
  for (const f of raw.fields) {
    if (!identifier(f.key) || keys.has(f.key) || typeof f.label !== 'string' ||
        !['date','text','number','select'].includes(f.type)) invalid()
    if (f.type === 'select' && (!f.options || !Object.keys(f.options).length || !Object.values(f.options).every(v => typeof v === 'string'))) invalid()
    keys.add(f.key)
  }
  const hasType = (key: string, type: Field['type']) => raw.fields.some(f => f.key === key && f.type === type)
  if (!hasType(raw.period.field,'date') || !hasType(raw.summary?.amount,'number') || !hasType(raw.summary.group,'select') ||
      raw.fields.some(f => f.when && !keys.has(f.when.key))) invalid()
  for (const label of ['create','edit','submit','history','empty','modify','delete','confirm','cancel'] as const) {
    if (typeof raw.labels?.[label] !== 'string') invalid()
  }
  new Intl.NumberFormat(raw.summary.locale, {style:'currency', currency:raw.summary.currency})
  return raw
}
