"use client";

/**
 * PixelChip — CSS-only pixel art poker chip sprites.
 * Four denominations: white (25), red (100), blue (500), gold (1000).
 */

type ChipColor = "white" | "red" | "blue" | "gold";

const CHIP_COLORS: Record<ChipColor, { outer: string; inner: string; edge: string; highlight: string }> = {
  white: { outer: "#e0e0e0", inner: "#ffffff", edge: "#bdbdbd", highlight: "#f5f5f5" },
  red:   { outer: "#c0392b", inner: "#e74c3c", edge: "#962d22", highlight: "#ff6b6b" },
  blue:  { outer: "#2471a3", inner: "#3498db", edge: "#1a5276", highlight: "#5dade2" },
  gold:  { outer: "#d4ac0d", inner: "#f1c40f", edge: "#b7950b", highlight: "#f4d03f" },
};

const CHIP_VALUES: Record<ChipColor, number> = {
  gold: 1000,
  blue: 500,
  red: 100,
  white: 25,
};

interface PixelChipProps {
  color?: ChipColor;
  size?: number;
}

export function PixelChip({ color = "red", size = 3 }: PixelChipProps) {
  const c = CHIP_COLORS[color];
  const px = size;

  return (
    <div style={{ display: "inline-block" }}>
      <div
        style={{
          width: `${px}px`,
          height: `${px}px`,
          background: "transparent",
          boxShadow: `
            /* Top edge */
            ${2 * px}px ${0 * px}px 0 ${c.edge},
            ${3 * px}px ${0 * px}px 0 ${c.edge},
            ${4 * px}px ${0 * px}px 0 ${c.edge},
            ${5 * px}px ${0 * px}px 0 ${c.edge},

            /* Row 1 */
            ${1 * px}px ${1 * px}px 0 ${c.edge},
            ${2 * px}px ${1 * px}px 0 ${c.outer},
            ${3 * px}px ${1 * px}px 0 ${c.highlight},
            ${4 * px}px ${1 * px}px 0 ${c.highlight},
            ${5 * px}px ${1 * px}px 0 ${c.outer},
            ${6 * px}px ${1 * px}px 0 ${c.edge},

            /* Row 2 */
            ${0 * px}px ${2 * px}px 0 ${c.edge},
            ${1 * px}px ${2 * px}px 0 ${c.outer},
            ${2 * px}px ${2 * px}px 0 ${c.highlight},
            ${3 * px}px ${2 * px}px 0 ${c.inner},
            ${4 * px}px ${2 * px}px 0 ${c.inner},
            ${5 * px}px ${2 * px}px 0 ${c.outer},
            ${6 * px}px ${2 * px}px 0 ${c.outer},
            ${7 * px}px ${2 * px}px 0 ${c.edge},

            /* Row 3 - center */
            ${0 * px}px ${3 * px}px 0 ${c.edge},
            ${1 * px}px ${3 * px}px 0 ${c.outer},
            ${2 * px}px ${3 * px}px 0 ${c.inner},
            ${3 * px}px ${3 * px}px 0 ${c.highlight},
            ${4 * px}px ${3 * px}px 0 ${c.highlight},
            ${5 * px}px ${3 * px}px 0 ${c.inner},
            ${6 * px}px ${3 * px}px 0 ${c.outer},
            ${7 * px}px ${3 * px}px 0 ${c.edge},

            /* Row 4 */
            ${0 * px}px ${4 * px}px 0 ${c.edge},
            ${1 * px}px ${4 * px}px 0 ${c.outer},
            ${2 * px}px ${4 * px}px 0 ${c.outer},
            ${3 * px}px ${4 * px}px 0 ${c.inner},
            ${4 * px}px ${4 * px}px 0 ${c.inner},
            ${5 * px}px ${4 * px}px 0 ${c.highlight},
            ${6 * px}px ${4 * px}px 0 ${c.outer},
            ${7 * px}px ${4 * px}px 0 ${c.edge},

            /* Row 5 */
            ${1 * px}px ${5 * px}px 0 ${c.edge},
            ${2 * px}px ${5 * px}px 0 ${c.outer},
            ${3 * px}px ${5 * px}px 0 ${c.outer},
            ${4 * px}px ${5 * px}px 0 ${c.outer},
            ${5 * px}px ${5 * px}px 0 ${c.outer},
            ${6 * px}px ${5 * px}px 0 ${c.edge},

            /* Bottom edge */
            ${2 * px}px ${6 * px}px 0 ${c.edge},
            ${3 * px}px ${6 * px}px 0 ${c.edge},
            ${4 * px}px ${6 * px}px 0 ${c.edge},
            ${5 * px}px ${6 * px}px 0 ${c.edge}
          `,
        }}
      />
    </div>
  );
}

function breakIntoDenominations(amount: number): ChipColor[] {
  const chips: ChipColor[] = [];
  const order: ChipColor[] = ["gold", "blue", "red", "white"];
  let remaining = amount;

  for (const color of order) {
    const value = CHIP_VALUES[color];
    while (remaining >= value && chips.length < 8) {
      chips.push(color);
      remaining -= value;
    }
  }

  if (chips.length === 0 && amount > 0) {
    chips.push("white");
  }

  return chips;
}

interface PixelChipStackProps {
  amount: number;
  size?: number;
}

export function PixelChipStack({ amount, size = 2 }: PixelChipStackProps) {
  const chips = breakIntoDenominations(amount);
  if (chips.length === 0) return null;

  return (
    <div
      style={{
        display: "inline-flex",
        flexDirection: "column-reverse",
        alignItems: "center",
        gap: "0px",
      }}
    >
      {chips.map((color, i) => (
        <div
          key={i}
          style={{
            marginTop: i > 0 ? `-${size * 4}px` : "0px",
            zIndex: i,
          }}
        >
          <PixelChip color={color} size={size} />
        </div>
      ))}
    </div>
  );
}

interface PotChipPileProps {
  amount: number;
  size?: number;
}

export function PotChipPile({ amount, size = 3 }: PotChipPileProps) {
  const chips = breakIntoDenominations(amount);
  if (chips.length === 0) return null;

  return (
    <div
      style={{
        display: "inline-flex",
        flexDirection: "row",
        alignItems: "flex-end",
        gap: `${size}px`,
      }}
    >
      {chips.map((color, i) => (
        <div
          key={i}
          style={{
            marginBottom: `${(i % 3) * size}px`,
          }}
        >
          <PixelChip color={color} size={size} />
        </div>
      ))}
    </div>
  );
}
