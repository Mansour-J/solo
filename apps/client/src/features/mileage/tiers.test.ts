import { formatTiers } from './format';
import { fromVersion, newRateSchema, parseLimitKm, rateVersionSchema, toNewRateInput, toRateVersionInput } from './rateForm';

describe('IRD’s two-tier kilometre rates', () => {
  it('reads a tier 1 limit as typed', () => {
    expect(parseLimitKm('14,000')).toBe('14000');
    expect(parseLimitKm('14000 km')).toBe('14000');
    expect(parseLimitKm('0')).toBeNull();
    expect(parseLimitKm('abc')).toBeNull();
  });

  it('needs both tier fields or neither', () => {
    const base = { effective_from: '2026-04-01', rate_per_km: '1.20' };
    expect(rateVersionSchema.safeParse(base).success).toBe(true);
    expect(rateVersionSchema.safeParse({ ...base, tier1_limit_km: '14000', tier2_rate_per_km: '0.37' }).success).toBe(true);
    const half = rateVersionSchema.safeParse({ ...base, tier1_limit_km: '14000', tier2_rate_per_km: '' });
    expect(half.error?.issues.map((i) => i.path.join('.'))).toEqual(['tier2_rate_per_km']);
    const named = newRateSchema.safeParse({ name: 'IRD', notes: '', ...base, tier1_limit_km: '', tier2_rate_per_km: '0.37' });
    expect(named.error?.issues.map((i) => i.path.join('.'))).toEqual(['tier1_limit_km']);
  });

  it('sends the tiers only when they’re filled in', () => {
    expect(toRateVersionInput({ effective_from: '2026-04-01', rate_per_km: '1.2', tier1_limit_km: '', tier2_rate_per_km: '' })).toEqual({
      effective_from: '2026-04-01',
      rate_per_km: '1.2',
    });
    expect(
      toNewRateInput({
        name: 'IRD',
        notes: '',
        effective_from: '2026-04-01',
        rate_per_km: '1.20',
        tier1_limit_km: '14,000',
        tier2_rate_per_km: '$0.37',
      }),
    ).toMatchObject({ tier1_limit_km: '14000', tier2_rate_per_km: '0.37' });
    expect(
      fromVersion({ id: 'v', effective_from: '2026-04-01', rate_per_km: '1.2000', tier1_limit_km: '14000.0', tier2_rate_per_km: '0.3700' }),
    ).toMatchObject({ tier1_limit_km: '14000', tier2_rate_per_km: '0.37' });
  });

  it('describes the tiers', () => {
    expect(formatTiers('14000.0', '0.3700', 'NZD')).toBe('$0.37/km after 14,000 km');
    expect(formatTiers(null, null)).toBe('');
  });
});
