// Typed client for the REST API. Paths, parameters and bodies are checked against the OpenAPI
// document generated from the Rust handlers, so the client cannot drift from the server.
import createClient, { type Client } from 'openapi-fetch';
import type { components, paths } from './schema';

export type { components, paths } from './schema';
export type ApiClient = Client<paths>;

type Schemas = components['schemas'];

export type AppMeta = Schemas['AppMeta'];
export type Brand = Schemas['Brand'];
export type UserProfile = Schemas['UserProfile'];
export type Preferences = Schemas['Preferences'];
export type Business = Schemas['Business'];
export type BusinessInput = Schemas['BusinessInput'];
export type BusinessSummary = Schemas['BusinessSummary'];
export type BusinessSettings = Schemas['BusinessSettings'];
export type BusinessType = Schemas['BusinessType'];
export type Address = Schemas['Address'];
export type AuditLogRow = Schemas['AuditLogRow'];
export type ErrorBody = Schemas['ErrorBody'];
export type TaxRateRow = Schemas['TaxRateRow'];

export interface ApiClientOptions {
  /** Origin of the API, e.g. `https://api.example.com`; empty for same-origin (`/api/...`). */
  baseUrl: string;
  /** A fetch implementation that adds authentication (cookies or bearer token) and refresh. */
  fetch: (request: Request) => Promise<Response>;
}

export function createApiClient({ baseUrl, fetch }: ApiClientOptions): ApiClient {
  return createClient<paths>({ baseUrl, fetch });
}
