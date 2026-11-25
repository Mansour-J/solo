import { calculableLines, documentSchema, emptyLine, newDocument, toLines } from './documentForm';
import { taxLabel } from './labels';

describe('invoice editor model', () => {
  it('starts with one line using GST when the business is registered', () => {
    const doc = newDocument('2026-11-20', true, 'customer-1', 'Thanks!', 'Small print');
    expect(doc.lines).toHaveLength(1);
    expect(doc.lines[0]?.tax_treatment).toBe('standard');
    expect(doc.notes).toBe('Thanks!');
    expect(newDocument('2026-11-20', false, null).lines[0]?.tax_treatment).toBe('no_gst');
  });

  it('only sends complete lines for the live calculation', () => {
    const complete = { ...emptyLine(true), description: 'Design', quantity: '7.5', unit_price: 12_500 };
    const missingPrice = { ...emptyLine(true), description: 'Pending' };
    const badQuantity = { ...emptyLine(true), quantity: 'two', unit_price: 100 };
    expect(calculableLines([complete, missingPrice, badQuantity])).toEqual([
      { description: 'Design', quantity: '7.5', unit_price: 12_500, discount_percent: '0', tax_treatment: 'standard' },
    ]);
  });

  it('validates lines with field paths the API also uses', () => {
    const doc = { ...newDocument('2026-11-20', true, null), lines: [{ ...emptyLine(true), quantity: '0', discount_percent: '150' }] };
    const result = documentSchema.safeParse(doc);
    const paths = result.error?.issues.map((i) => i.path.join('.')) ?? [];
    expect(paths).toEqual(
      expect.arrayContaining(['customer_id', 'lines.0.description', 'lines.0.quantity', 'lines.0.unit_price', 'lines.0.discount_percent']),
    );
  });

  it('rejects a due date before the issue date', () => {
    const doc = {
      ...newDocument('2026-11-20', true, 'c'),
      end_date: '2026-11-19',
      lines: [{ ...emptyLine(true), description: 'x', unit_price: 1 }],
    };
    expect(documentSchema.safeParse(doc).error?.issues[0]?.path).toEqual(['end_date']);
  });

  it('trims text and defaults empty discounts', () => {
    expect(toLines([{ ...emptyLine(false), description: '  Logo  ', quantity: ' 1 ', unit_price: 5_000, discount_percent: '' }])).toEqual([
      { description: 'Logo', quantity: '1', unit_price: 5_000, discount_percent: '0', tax_treatment: 'no_gst' },
    ]);
  });

  it('labels GST rates without floating-point noise', () => {
    expect(taxLabel('standard', '0.150000')).toBe('15%');
    expect(taxLabel('standard', '0.125')).toBe('12.5%');
    expect(taxLabel('exempt', '0')).toBe('Exempt');
  });
});
