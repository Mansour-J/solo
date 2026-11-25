import { formatDate, formatGstNumber, formatMoney, minorToDecimalString, parseMoney, setFormatPreferences } from './format';

describe('money', () => {
  it('parses user input into integer minor units without floating-point error', () => {
    expect(parseMoney('1,234.56')).toBe(123456);
    expect(parseMoney('$20')).toBe(2000);
    expect(parseMoney('0.1')).toBe(10);
    expect(parseMoney('19.99')).toBe(1999);
    expect(parseMoney('-5.5')).toBe(-550);
    expect(parseMoney('.75')).toBe(75);
    expect(parseMoney(12.5)).toBe(1250);
  });

  it('rejects malformed amounts and extra precision', () => {
    expect(parseMoney('')).toBeNull();
    expect(parseMoney('abc')).toBeNull();
    expect(parseMoney('1.234')).toBeNull();
    expect(parseMoney('1.2.3')).toBeNull();
    expect(parseMoney('1.5', 'JPY')).toBeNull();
    expect(parseMoney('1500', 'JPY')).toBe(1500);
  });

  it('formats minor units for display', () => {
    setFormatPreferences({ date_format: 'DD/MM/YYYY', number_locale: 'en-NZ' });
    expect(formatMoney(123456)).toBe('$1,234.56');
    expect(formatMoney(-500)).toBe('-$5.00');
    expect(formatMoney(1500, 'JPY')).toBe('¥1,500');
  });

  it('round-trips minor units through input strings', () => {
    expect(minorToDecimalString(123456)).toBe('1234.56');
    expect(minorToDecimalString(-5)).toBe('-0.05');
    expect(parseMoney(minorToDecimalString(987654321))).toBe(987654321);
  });
});

describe('dates', () => {
  it('follows the user’s date format preference', () => {
    setFormatPreferences({ date_format: 'DD/MM/YYYY', number_locale: 'en-NZ' });
    expect(formatDate('2026-03-31')).toBe('31/03/2026');
    setFormatPreferences({ date_format: 'YYYY-MM-DD', number_locale: 'en-NZ' });
    expect(formatDate('2026-03-31')).toBe('2026-03-31');
    expect(formatDate(null)).toBe('—');
    setFormatPreferences({ date_format: 'DD/MM/YYYY', number_locale: 'en-NZ' });
  });
});

describe('GST numbers', () => {
  it('groups digits the way IRD writes them', () => {
    expect(formatGstNumber('49091850')).toBe('49-091-850');
    expect(formatGstNumber('136410132')).toBe('136-410-132');
  });
});
