// The harness's switches (?as=, ?still=, ?scenario=): read from the address the tab was opened at and kept for the tab,
// so a reload at a page's own path (/notes?view=reviews) still has them.
const params = new URLSearchParams(location.search)
export function flag(name: string): string | null {
  const v = params.get(name)
  if (v !== null) sessionStorage.setItem(`verify.${name}`, v)
  return v ?? sessionStorage.getItem(`verify.${name}`)
}
