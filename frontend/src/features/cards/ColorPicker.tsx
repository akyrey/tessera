import { CARD_COLORS } from './cardColors'

interface ColorPickerProps {
  /** `null` = automatic colour from the store name. */
  readonly value: string | null
  readonly onChange: (color: string | null) => void
}

export function ColorPicker({ value, onChange }: ColorPickerProps) {
  return (
    <fieldset className="flex flex-col gap-2">
      <legend className="font-medium">Colour</legend>
      <div className="flex flex-wrap gap-2">
        <label className="flex min-h-11 cursor-pointer items-center rounded-full px-3 text-sm ring-1 ring-brand-900/15 has-checked:ring-2 has-checked:ring-brand-700">
          <input
            type="radio"
            name="color"
            className="sr-only"
            checked={value === null}
            onChange={() => onChange(null)}
          />
          Auto
        </label>
        {CARD_COLORS.map((color) => (
          <label
            key={color}
            className="size-11 cursor-pointer rounded-full ring-offset-2 has-checked:ring-2 has-checked:ring-brand-900"
            style={{ backgroundColor: color }}
          >
            <input
              type="radio"
              name="color"
              aria-label={color}
              className="sr-only"
              checked={value === color}
              onChange={() => onChange(color)}
            />
          </label>
        ))}
      </div>
    </fieldset>
  )
}
