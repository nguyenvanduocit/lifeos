// Reading a view: `useRead` runs a read tool and runs it again whenever the
// server says the data changed, or when its input changes.
import { computed, ref, shallowRef, watch, type Ref } from 'vue'
import { read, revision, type ReadTool, type ReadTools } from './core/link'

export interface Reading<T> {
  data: Ref<T | undefined>
  error: Ref<string>
  loading: Ref<boolean>
  retry: () => void
}

export function useRead<T extends ReadTool>(
  tool: T,
  input: () => ReadTools[T]['input'] = () => ({}) as ReadTools[T]['input'],
): Reading<ReadTools[T]['output']> {
  const data = shallowRef<ReadTools[T]['output']>()
  const error = ref('')
  const loading = ref(true)
  const key = computed(() => JSON.stringify(input() ?? {}))
  const attempt = ref(0)
  let seq = 0
  watch(
    [key, revision, attempt],
    async ([inputKey], [previousKey]) => {
      if (inputKey !== previousKey) data.value = undefined
      error.value = ''
      const mine = ++seq
      loading.value = true
      try {
        const out = await read(tool, JSON.parse(inputKey))
        if (mine === seq) {
          data.value = out
          error.value = ''
        }
      } catch (err) {
        if (mine === seq) error.value = err instanceof Error ? err.message : String(err)
      } finally {
        if (mine === seq) loading.value = false
      }
    },
    { immediate: true },
  )
  return { data, error, loading, retry: () => { attempt.value++ } }
}
