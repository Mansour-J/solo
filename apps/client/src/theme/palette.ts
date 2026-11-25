import type { MantineColorsTuple } from '@mantine/core';

function parseHex(hex: string): [number, number, number] {
  const value = hex.replace('#', '');
  return [0, 2, 4].map((i) => parseInt(value.slice(i, i + 2), 16)) as [number, number, number];
}

function toHex([r, g, b]: [number, number, number]): string {
  return `#${[r, g, b]
    .map((c) =>
      Math.round(Math.min(255, Math.max(0, c)))
        .toString(16)
        .padStart(2, '0'),
    )
    .join('')}`;
}

function mix(a: [number, number, number], b: [number, number, number], amount: number): [number, number, number] {
  return [a[0] + (b[0] - a[0]) * amount, a[1] + (b[1] - a[1]) * amount, a[2] + (b[2] - a[2]) * amount];
}

/**
 * A ten-shade Mantine palette around a brand colour: shade 6 is the colour itself, lighter shades
 * mix towards white and darker shades towards black. Lets any configured brand colour drive the theme.
 */
export function generatePalette(hex: string): MantineColorsTuple {
  const base = parseHex(hex);
  const white: [number, number, number] = [255, 255, 255];
  const black: [number, number, number] = [0, 0, 0];
  const lighter = [0.93, 0.85, 0.7, 0.52, 0.34, 0.16].map((amount) => toHex(mix(base, white, amount)));
  const darker = [0.14, 0.28, 0.42].map((amount) => toHex(mix(base, black, amount)));
  return [...lighter, hex.toLowerCase(), ...darker] as unknown as MantineColorsTuple;
}
