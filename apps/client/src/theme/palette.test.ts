import { generatePalette } from './palette';

describe('generatePalette', () => {
  it('builds ten shades around the brand colour', () => {
    const palette = generatePalette('#13795B');
    expect(palette).toHaveLength(10);
    expect(palette[6]).toBe('#13795b');
    for (const shade of palette) expect(shade).toMatch(/^#[0-9a-f]{6}$/);
  });

  it('goes from light to dark', () => {
    const brightness = (hex: string) => [1, 3, 5].reduce((sum, i) => sum + parseInt(hex.slice(i, i + 2), 16), 0);
    const palette = generatePalette('#c2410c');
    for (let i = 1; i < palette.length; i++) expect(brightness(palette[i]!)).toBeLessThan(brightness(palette[i - 1]!));
  });
});
