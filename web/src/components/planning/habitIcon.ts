// An icon for a habit, guessed from its name (English or Vietnamese); a sprout when nothing matches.
import type { Component } from 'vue'
import {
  BedDouble, Bike, BookOpen, Brush, CandyOff, Droplets, Dumbbell, Flower2, Footprints, Languages, Music, PenLine, Salad, Smartphone, Sprout, Wind,
} from '@lucide/vue'

const RULES: [RegExp, Component][] = [
  [/meditat|thiền|mindful|breath|thở/i, Flower2],
  [/read|đọc|book|sách/i, BookOpen],
  [/run|chạy|jog|walk|đi bộ|step|bước/i, Footprints],
  [/bike|cycl|đạp xe/i, Bike],
  [/gym|workout|lift|tập|push|exercise|thể dục|yoga|stretch/i, Dumbbell],
  [/sugar|đường|candy|sweet|ngọt/i, CandyOff],
  [/water|nước|drink|uống/i, Droplets],
  [/sleep|ngủ|bed/i, BedDouble],
  [/writ|viết|journal|nhật ký/i, PenLine],
  [/vegetable|rau|salad|eat|ăn/i, Salad],
  [/phone|screen|điện thoại|social/i, Smartphone],
  [/music|piano|guitar|nhạc|đàn/i, Music],
  [/english|language|tiếng|học/i, Languages],
  [/draw|paint|vẽ|art/i, Brush],
  [/cold|air|outside|ngoài trời/i, Wind],
]

export function habitIcon(name: string): Component {
  return RULES.find(([re]) => re.test(name))?.[1] ?? Sprout
}
