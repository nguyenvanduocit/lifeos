// What a stream needs to know beyond its own events, as the panel that shows it says: an agent's name by its id, and
// where an agent stands (the receipt under a message reads which message it is on).
import { inject, provide, type InjectionKey } from 'vue'
import type { AgentStanding } from '../../core/generated/AgentStanding'

export interface StreamContext {
  nameOf: (id: string) => string
  standingOf: (id: string) => AgentStanding | undefined
}
const KEY: InjectionKey<StreamContext> = Symbol('stream')
const NOWHERE: StreamContext = { nameOf: (id) => id, standingOf: () => undefined }
export const provideStream = (c: StreamContext): void => provide(KEY, c)
export const useStream = (): StreamContext => inject(KEY, NOWHERE)

// Where a memory opens: the drawer that shows the stream takes its Memory tab to it. Outside a drawer nothing opens.
const MEMORY: InjectionKey<(id: number) => void> = Symbol('openMemory')
export const provideOpenMemory = (open: (id: number) => void): void => provide(MEMORY, open)
export const useOpenMemory = (): ((id: number) => void) | undefined => inject(MEMORY, undefined)
