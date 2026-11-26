// Keeps the stand-in honest: `npm run typecheck` fails here when src/core/link.ts has an export fakeLink.ts lacks, or
// one whose type the fake's does not fit. (Types only; nothing of this file reaches a page.)
import type * as Real from '../src/core/link'
import type * as Fake from './fakeLink'

type Missing = Exclude<keyof typeof Real, keyof typeof Fake>
export const missing: [Missing] extends [never] ? true : Missing = true
export const fits = (fake: typeof Fake): typeof Real => fake
