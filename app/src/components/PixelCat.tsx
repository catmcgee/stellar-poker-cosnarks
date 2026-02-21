"use client";

/**
 * PixelCat — CSS-only pixel art cat sprites.
 * Three variants: grey tabby, orange tabby, black cat.
 * These are placeholders — user will provide custom cat images later.
 */

type CatVariant = "grey" | "orange" | "black";

interface PixelCatProps {
  variant?: CatVariant;
  size?: number;
  idle?: boolean;
  flipped?: boolean;
}

const CAT_COLORS: Record<CatVariant, { body: string; dark: string; light: string; belly: string; eyes: string }> = {
  grey: {
    body: '#9e9e9e',
    dark: '#757575',
    light: '#bdbdbd',
    belly: '#e0e0e0',
    eyes: '#4caf50',
  },
  orange: {
    body: '#e67e22',
    dark: '#d35400',
    light: '#f0a04b',
    belly: '#fdebd0',
    eyes: '#27ae60',
  },
  black: {
    body: '#2c2c2c',
    dark: '#1a1a1a',
    light: '#444444',
    belly: '#555555',
    eyes: '#f1c40f',
  },
};

export function PixelCat({ variant = "orange", size = 4, idle = true, flipped = false }: PixelCatProps) {
  const c = CAT_COLORS[variant];
  const px = size;

  return (
    <div
      style={{
        display: 'inline-block',
        animation: idle ? 'catIdle 2s ease-in-out infinite' : undefined,
        transform: flipped ? 'scaleX(-1)' : undefined,
      }}
    >
      <div style={{
        width: `${px}px`,
        height: `${px}px`,
        background: 'transparent',
        boxShadow: `
          /* Ears */
          ${2*px}px ${0*px}px 0 ${c.dark},
          ${3*px}px ${0*px}px 0 ${c.body},
          ${7*px}px ${0*px}px 0 ${c.body},
          ${8*px}px ${0*px}px 0 ${c.dark},

          ${2*px}px ${1*px}px 0 ${c.body},
          ${3*px}px ${1*px}px 0 ${c.light},
          ${7*px}px ${1*px}px 0 ${c.light},
          ${8*px}px ${1*px}px 0 ${c.body},

          /* Head top */
          ${3*px}px ${2*px}px 0 ${c.body},
          ${4*px}px ${2*px}px 0 ${c.body},
          ${5*px}px ${2*px}px 0 ${c.body},
          ${6*px}px ${2*px}px 0 ${c.body},
          ${7*px}px ${2*px}px 0 ${c.body},

          /* Head row with eyes */
          ${2*px}px ${3*px}px 0 ${c.body},
          ${3*px}px ${3*px}px 0 ${c.eyes},
          ${4*px}px ${3*px}px 0 ${c.body},
          ${5*px}px ${3*px}px 0 ${c.body},
          ${6*px}px ${3*px}px 0 ${c.body},
          ${7*px}px ${3*px}px 0 ${c.eyes},
          ${8*px}px ${3*px}px 0 ${c.body},

          /* Nose/mouth row */
          ${2*px}px ${4*px}px 0 ${c.body},
          ${3*px}px ${4*px}px 0 ${c.body},
          ${4*px}px ${4*px}px 0 ${c.body},
          ${5*px}px ${4*px}px 0 #ffb6c1,
          ${6*px}px ${4*px}px 0 ${c.body},
          ${7*px}px ${4*px}px 0 ${c.body},
          ${8*px}px ${4*px}px 0 ${c.body},

          /* Whisker row */
          ${0*px}px ${4*px}px 0 ${c.dark},
          ${1*px}px ${3*px}px 0 ${c.dark},
          ${9*px}px ${3*px}px 0 ${c.dark},
          ${10*px}px ${4*px}px 0 ${c.dark},

          /* Body */
          ${3*px}px ${5*px}px 0 ${c.body},
          ${4*px}px ${5*px}px 0 ${c.belly},
          ${5*px}px ${5*px}px 0 ${c.belly},
          ${6*px}px ${5*px}px 0 ${c.belly},
          ${7*px}px ${5*px}px 0 ${c.body},

          ${2*px}px ${6*px}px 0 ${c.body},
          ${3*px}px ${6*px}px 0 ${c.body},
          ${4*px}px ${6*px}px 0 ${c.belly},
          ${5*px}px ${6*px}px 0 ${c.belly},
          ${6*px}px ${6*px}px 0 ${c.belly},
          ${7*px}px ${6*px}px 0 ${c.body},
          ${8*px}px ${6*px}px 0 ${c.body},

          ${2*px}px ${7*px}px 0 ${c.body},
          ${3*px}px ${7*px}px 0 ${c.belly},
          ${4*px}px ${7*px}px 0 ${c.belly},
          ${5*px}px ${7*px}px 0 ${c.belly},
          ${6*px}px ${7*px}px 0 ${c.belly},
          ${7*px}px ${7*px}px 0 ${c.belly},
          ${8*px}px ${7*px}px 0 ${c.body},

          /* Legs */
          ${2*px}px ${8*px}px 0 ${c.body},
          ${3*px}px ${8*px}px 0 ${c.body},
          ${4*px}px ${8*px}px 0 ${c.belly},
          ${5*px}px ${8*px}px 0 ${c.belly},
          ${6*px}px ${8*px}px 0 ${c.belly},
          ${7*px}px ${8*px}px 0 ${c.body},
          ${8*px}px ${8*px}px 0 ${c.body},

          /* Paws */
          ${2*px}px ${9*px}px 0 ${c.dark},
          ${3*px}px ${9*px}px 0 ${c.dark},
          ${7*px}px ${9*px}px 0 ${c.dark},
          ${8*px}px ${9*px}px 0 ${c.dark},

          /* Tail */
          ${9*px}px ${6*px}px 0 ${c.body},
          ${10*px}px ${5*px}px 0 ${c.body},
          ${11*px}px ${4*px}px 0 ${c.body},
          ${11*px}px ${3*px}px 0 ${c.dark}
        `,
      }} />
    </div>
  );
}

export function PixelHeart({ size = 4, beating = false }: { size?: number; beating?: boolean }) {
  const px = size;
  return (
    <div style={{
      display: 'inline-block',
      animation: beating ? 'heartBeat 1s ease-in-out infinite' : undefined,
    }}>
      <div style={{
        width: `${px}px`,
        height: `${px}px`,
        background: 'transparent',
        boxShadow: `
          ${1*px}px ${0}px 0 #e74c3c,
          ${2*px}px ${0}px 0 #e74c3c,
          ${4*px}px ${0}px 0 #e74c3c,
          ${5*px}px ${0}px 0 #e74c3c,
          ${0}px ${1*px}px 0 #e74c3c,
          ${1*px}px ${1*px}px 0 #ff6b6b,
          ${2*px}px ${1*px}px 0 #e74c3c,
          ${3*px}px ${1*px}px 0 #e74c3c,
          ${4*px}px ${1*px}px 0 #e74c3c,
          ${5*px}px ${1*px}px 0 #e74c3c,
          ${6*px}px ${1*px}px 0 #e74c3c,
          ${0}px ${2*px}px 0 #e74c3c,
          ${1*px}px ${2*px}px 0 #e74c3c,
          ${2*px}px ${2*px}px 0 #e74c3c,
          ${3*px}px ${2*px}px 0 #e74c3c,
          ${4*px}px ${2*px}px 0 #e74c3c,
          ${5*px}px ${2*px}px 0 #e74c3c,
          ${6*px}px ${2*px}px 0 #c0392b,
          ${1*px}px ${3*px}px 0 #e74c3c,
          ${2*px}px ${3*px}px 0 #e74c3c,
          ${3*px}px ${3*px}px 0 #e74c3c,
          ${4*px}px ${3*px}px 0 #e74c3c,
          ${5*px}px ${3*px}px 0 #c0392b,
          ${2*px}px ${4*px}px 0 #e74c3c,
          ${3*px}px ${4*px}px 0 #e74c3c,
          ${4*px}px ${4*px}px 0 #c0392b,
          ${3*px}px ${5*px}px 0 #c0392b
        `,
      }} />
    </div>
  );
}
