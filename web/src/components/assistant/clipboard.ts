// Puts text on the clipboard, and says whether it went there. A page that is not served securely has no clipboard to write to.
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch {
    return false
  }
}
