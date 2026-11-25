// The customer form: values, validation (mirroring the API) and conversion to and from the API shape.
import type { components } from '@solo/api-client';
import { z } from 'zod';

export type Customer = components['schemas']['Customer'];
export type CustomerDetail = components['schemas']['CustomerDetail'];
export type CustomerInput = components['schemas']['CustomerInput'];
type Address = components['schemas']['Address'];

export interface AddressValues {
  line1: string;
  line2: string;
  suburb: string;
  city: string;
  region: string;
  postcode: string;
  country: string;
}

export interface CustomerFormValues {
  name: string;
  business_name: string;
  contact_person: string;
  email: string;
  phone: string;
  billing_address: AddressValues;
  business_address: AddressValues;
  nzbn: string;
  tax_number: string;
  /** Empty uses the business default. NumberInput reports '' when cleared. */
  payment_terms_days: number | string;
  notes: string;
  is_active: boolean;
}

export const emptyAddress: AddressValues = { line1: '', line2: '', suburb: '', city: '', region: '', postcode: '', country: 'NZ' };

export const emptyCustomerForm: CustomerFormValues = {
  name: '',
  business_name: '',
  contact_person: '',
  email: '',
  phone: '',
  billing_address: emptyAddress,
  business_address: emptyAddress,
  nzbn: '',
  tax_number: '',
  payment_terms_days: '',
  notes: '',
  is_active: true,
};

/** A 13-digit NZBN with a valid GS1 check digit (spaces and dashes allowed). */
export function isValidNzbn(value: string): boolean {
  const digits = value.replace(/[\s-]/g, '');
  if (!/^\d{13}$/.test(digits)) return false;
  const numbers = [...digits].map(Number);
  const sum = numbers.slice(0, 12).reduce((total, digit, i) => total + (i % 2 === 0 ? digit : digit * 3), 0);
  return (10 - (sum % 10)) % 10 === numbers[12];
}

const addressField = z.string().refine((v) => v.trim().length <= 120, 'Use 120 characters or fewer.');
const addressSchema = z.object({
  line1: addressField,
  line2: addressField,
  suburb: addressField,
  city: addressField,
  region: addressField,
  postcode: addressField,
  country: addressField,
});
const maxLength = (max: number) => z.string().refine((v) => v.trim().length <= max, `Use ${max} characters or fewer.`);

export const customerSchema = z
  .object({
    name: z.string().trim().min(1, 'Enter the customer’s name.').max(200, 'Use 200 characters or fewer.'),
    business_name: maxLength(200),
    contact_person: maxLength(200),
    email: z.union([z.literal(''), z.string().trim().email('Enter a valid email address.')]),
    phone: z
      .string()
      .refine((v) => v.trim().length <= 40, 'Use 40 characters or fewer.')
      .refine((v) => /^[\d\s+()\-.]*$/.test(v.trim()), 'Enter a phone number using digits, spaces and + ( ) - only.'),
    billing_address: addressSchema,
    business_address: addressSchema,
    nzbn: z.string().refine((v) => v.trim() === '' || isValidNzbn(v.trim()), 'Enter a valid 13-digit NZBN.'),
    tax_number: maxLength(40),
    payment_terms_days: z
      .unknown()
      .refine((v) => v === '' || v === null || (typeof v === 'number' && Number.isInteger(v) && v >= 0 && v <= 365), 'Use 0 to 365 days.'),
    notes: maxLength(4000),
  })
  .passthrough();

function toAddressValues(address: Address | undefined): AddressValues {
  return { ...emptyAddress, ...address };
}

export function fromCustomer(customer: Customer): CustomerFormValues {
  return {
    name: customer.name,
    business_name: customer.business_name,
    contact_person: customer.contact_person,
    email: customer.email ?? '',
    phone: customer.phone ?? '',
    billing_address: toAddressValues(customer.billing_address),
    business_address: toAddressValues(customer.business_address),
    nzbn: customer.nzbn ?? '',
    tax_number: customer.tax_number ?? '',
    payment_terms_days: customer.payment_terms_days ?? '',
    notes: customer.notes,
    is_active: customer.is_active,
  };
}

/** `existing` keeps fields the form doesn’t edit (the customer’s default currency). */
export function toCustomerInput(values: CustomerFormValues, existing?: Customer): CustomerInput {
  const terms = values.payment_terms_days;
  return {
    name: values.name.trim(),
    business_name: values.business_name.trim(),
    contact_person: values.contact_person.trim(),
    email: values.email.trim() || null,
    phone: values.phone.trim() || null,
    billing_address: values.billing_address,
    business_address: values.business_address,
    nzbn: values.nzbn.trim() || null,
    tax_number: values.tax_number.trim() || null,
    payment_terms_days: typeof terms === 'number' ? terms : null,
    notes: values.notes.trim(),
    default_currency: existing?.default_currency ?? null,
    is_active: values.is_active,
  };
}

/** The full input for an existing customer with only the active flag changed. */
export function withActive(customer: Customer, isActive: boolean): CustomerInput {
  return { ...toCustomerInput(fromCustomer(customer), customer), is_active: isActive };
}

/** Address lines for display, in the order they’re printed on documents. */
export function addressLines(address: Address | undefined): string[] {
  if (!address) return [];
  const cityLine = [address.city, address.postcode].filter(Boolean).join(' ');
  const country = address.country && address.country !== 'NZ' ? address.country : '';
  return [address.line1, address.line2, address.suburb, cityLine, address.region, country].map((l) => l.trim()).filter(Boolean);
}

export const STATUS_FILTERS = [
  { value: 'active', label: 'Active' },
  { value: 'inactive', label: 'Inactive' },
  { value: 'all', label: 'All' },
] as const;

export const SORT_OPTIONS = [
  { value: 'name', label: 'Name' },
  { value: 'outstanding', label: 'Highest balance' },
  { value: 'created', label: 'Newest' },
] as const;
