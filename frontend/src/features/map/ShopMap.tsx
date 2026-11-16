import 'leaflet/dist/leaflet.css'
import L from 'leaflet'
import { useEffect, useRef } from 'react'
import type { ShopMarker } from './mapMarkers'
import type { LatLon } from '../nearby/nearby'

const TILE_URL = 'https://tile.openstreetmap.org/{z}/{x}/{y}.png'
const ATTRIBUTION = '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors'
/** Italy, when there is neither a position nor any shop to frame. */
const DEFAULT_VIEW: L.LatLngExpression = [42.5, 12.5]
const DEFAULT_ZOOM = 5
const POSITION_ZOOM = 15

interface ShopMapProps {
  readonly markers: readonly ShopMarker[]
  readonly position: LatLon | null
  readonly onOpenCard: (cardId: string) => void
}

/** Leaflet map with one dot per shop; tapping a dot offers its card. */
export default function ShopMap({ markers, position, onOpenCard }: ShopMapProps) {
  const container = useRef<HTMLDivElement>(null)
  const open = useRef(onOpenCard)
  useEffect(() => {
    open.current = onOpenCard
  })

  useEffect(() => {
    if (!container.current) return
    // Canvas draws thousands of shops quickly.
    const map = L.map(container.current, { preferCanvas: true })
    L.tileLayer(TILE_URL, {
      maxZoom: 19,
      attribution: ATTRIBUTION,
      // OSM's tile policy asks for a referrer; send only our origin.
      referrerPolicy: 'strict-origin-when-cross-origin',
    }).addTo(map)

    for (const marker of markers) {
      L.circleMarker([marker.lat, marker.lon], {
        radius: 7,
        weight: 2,
        color: '#ffffff',
        fillColor: marker.source === 'place' ? '#be123c' : '#0f766e',
        fillOpacity: 0.9,
      })
        .bindPopup(() => popupFor(marker, (id) => open.current(id)))
        .addTo(map)
    }
    if (position) {
      L.circleMarker([position.lat, position.lon], { radius: 9, color: '#1d4ed8', fillOpacity: 0.6 }).addTo(
        map,
      )
      map.setView([position.lat, position.lon], POSITION_ZOOM)
    } else if (markers.length > 0) {
      map.fitBounds(L.latLngBounds(markers.map((m) => [m.lat, m.lon] as L.LatLngTuple)), {
        padding: [24, 24],
      })
    } else {
      map.setView(DEFAULT_VIEW, DEFAULT_ZOOM)
    }
    return () => {
      map.remove()
    }
  }, [markers, position])

  return <div ref={container} aria-label="Map of shops" className="min-h-0 flex-1" />
}

/** Built from DOM nodes, never HTML strings: store names are user input. */
function popupFor(marker: ShopMarker, onOpen: (cardId: string) => void): HTMLElement {
  const button = document.createElement('button')
  button.type = 'button'
  button.className = 'font-medium text-brand-700 underline'
  button.textContent = `Open ${marker.storeName}`
  button.addEventListener('click', () => onOpen(marker.cardId))
  return button
}
