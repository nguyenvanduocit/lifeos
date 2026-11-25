// A metric's figures over a range, and the change between two values written with its sign.
import { num } from '../../format'

export function stats(points: { value: number }[]) {
  if (!points.length) return null
  const v = points.map((p) => p.value)
  return { avg: v.reduce((a, b) => a + b, 0) / v.length, min: Math.min(...v), max: Math.max(...v), change: v[v.length - 1] - v[0] }
}

export function signed(n: number): string {
  if (Math.abs(n) < 1e-9) return '±0'
  return (n > 0 ? '+' : '−') + num(Math.abs(Math.round(n * 100) / 100))
}

export const RANGES = ['7', '30', '90', '365'] as const
export const rangeLabel = (r: string) => (r === '365' ? '1 year' : `${r} days`)
