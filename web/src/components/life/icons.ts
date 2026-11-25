// Icons and hues for the words the owner's records use: money categories, health metrics, ways of keeping in touch.
// Names come in English or Vietnamese, with or without accents.
import type { Component } from 'vue'
import {
  Activity, Banknote, Bus, Car, Coffee, Dumbbell, Footprints, Gift, GraduationCap, HandCoins, Heart, House, Mail, MessageCircle,
  Moon, Phone, Plane, Receipt, Scale, ShoppingBag, Smile, Stethoscope, Tag, Users, Utensils, Video, Wifi,
} from '@lucide/vue'

function plain(s: string): string {
  return s.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/đ/g, 'd').trim()
}

type Rule = [RegExp, Component]
function pick(rules: Rule[], fallback: Component) {
  return (name: string | null | undefined): Component => {
    const p = plain(name ?? '')
    return rules.find(([re]) => re.test(p))?.[1] ?? fallback
  }
}

export const categoryIcon = pick([
  [/coffee|ca phe|cafe|tra sua/, Coffee],
  [/food|\beat|an uong|\ban\b|\bcom\b|grocer|di cho|restaurant|lunch|dinner/, Utensils],
  [/taxi|grab|car|xang|fuel|parking|gui xe/, Car],
  [/transport|di lai|bus|xe|commute|metro/, Bus],
  [/shop|mua sam|clothes|quan ao/, ShoppingBag],
  [/bill|hoa don|dien|nuoc|utilit|internet|phone|rent|thue nha/, Receipt],
  [/salary|luong|wage|income|thu nhap|bonus|thuong/, Banknote],
  [/home|nha|house/, House],
  [/health|suc khoe|doctor|thuoc|medic|hospital/, Stethoscope],
  [/gym|sport|the thao|fitness/, Dumbbell],
  [/travel|du lich|flight|hotel|ve may bay/, Plane],
  [/gift|qua|tang/, Gift],
  [/learn|hoc|course|book|sach|education/, GraduationCap],
  [/subscri|wifi|cloud|app/, Wifi],
  [/lend|loan|vay|muon|invest|dau tu/, HandCoins],
], Tag)

export const metricIcon = pick([
  [/weight|can nang|kg/, Scale],
  [/sleep|giac ngu|ngu/, Moon],
  [/mood|tam trang|cam xuc/, Smile],
  [/step|buoc chan|buoc/, Footprints],
  [/heart|tim|pulse|nhip/, Heart],
  [/run|chay|workout|tap/, Dumbbell],
], Activity)

/** A metric's own hue (an area's tone key), so a page of them is not one colour. */
export function metricTone(name: string): string {
  const p = plain(name)
  if (/sleep|giac ngu|ngu/.test(p)) return 'automation'
  if (/mood|tam trang|cam xuc/.test(p)) return 'goals'
  if (/step|buoc|run|chay/.test(p)) return 'tasks'
  if (/weight|can nang/.test(p)) return 'people'
  return 'health'
}

export const channelIcon = pick([
  [/call|goi|phone|dien thoai/, Phone],
  [/video|zoom|google meet|facetime/, Video],
  [/mail|email|thu/, Mail],
  [/message|nhan|chat|text|zalo|messenger|sms|tin/, MessageCircle],
  [/coffee|cafe|ca phe/, Coffee],
  [/meet|gap|visit|tham|an toi|dinner|lunch/, Users],
], MessageCircle)

/** Hues for a set of categories, in order (largest first); they stay distinct up to eight. */
export const SERIES_TONES = ['money', 'goals', 'health', 'people', 'habits', 'inbox', 'tasks', 'journal'] as const
