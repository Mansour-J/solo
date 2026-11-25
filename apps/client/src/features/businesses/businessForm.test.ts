import type { Business } from '@solo/api-client';
import { businessSchema, emptyBusinessForm, fromBusiness, toBusinessInput } from './businessForm';

describe('business form', () => {
  it('sends blank optional fields as null and drops GST details when not registered', () => {
    const input = toBusinessInput({ ...emptyBusinessForm, name: ' Kōwhai Design ', gst_number: '49091850', gst_registered: false }, true);
    expect(input.name).toBe('Kōwhai Design');
    expect(input.email).toBeNull();
    expect(input.gst_number).toBeNull();
    expect(input.financial_year_start_month).toBe(4);
    expect(input.complete_onboarding).toBe(true);
  });

  it('round-trips a business from the API', () => {
    const business = {
      id: 'b1',
      name: 'Harbour Consulting',
      trading_name: 'Mere Smith',
      business_type: 'contractor',
      email: 'mere@example.nz',
      phone: null,
      address: { line1: '1 Queen St', line2: '', suburb: '', city: 'Auckland', region: '', postcode: '1010', country: 'NZ' },
      nzbn: null,
      gst_registered: true,
      gst_number: '136410132',
      gst_registered_from: '2024-04-01',
      gst_basis: 'invoice',
      currency: 'NZD',
      financial_year_start_month: 4,
      financial_year_start_day: 1,
      reporting_basis: 'cash',
    } as unknown as Business;
    const input = toBusinessInput(fromBusiness(business));
    expect(input).toMatchObject({
      name: 'Harbour Consulting',
      business_type: 'contractor',
      phone: null,
      gst_number: '136410132',
      gst_registered_from: '2024-04-01',
      gst_basis: 'invoice',
      reporting_basis: 'cash',
    });
  });

  it('validates identifiers before they reach the server', () => {
    const bad = businessSchema.safeParse({ ...emptyBusinessForm, name: 'X', nzbn: '123', gst_registered: true, gst_number: '12' });
    const paths = bad.error?.issues.map((i) => i.path.join('.')) ?? [];
    expect(paths).toEqual(expect.arrayContaining(['nzbn', 'gst_number']));
    expect(businessSchema.safeParse({ ...emptyBusinessForm, name: 'X', nzbn: '9429-0412-3456-3' }).success).toBe(true);
  });
});
