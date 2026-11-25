import type { InjectionKey, Ref } from 'vue'
import type { CustomWidget } from './core/generated/CustomWidget'
import type { WidgetGrid } from './core/generated/WidgetGrid'

export const pageWidgets: InjectionKey<Readonly<Ref<CustomWidget[] | undefined>>> = Symbol('pageWidgets')

export type Placement = { id: number; grid: WidgetGrid }
export const gridColumns = 48
export const gridRows = 16384
export const rowHeight = 16
export const gridGap = 8
