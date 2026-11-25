import type { FormErrors } from '@mantine/form';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import type { z } from 'zod';

/**
 * A Mantine form validator from a zod schema, typed by the form's values. Schemas may validate only
 * some fields (with `.passthrough()`), so the resolver's inferred output type is not the form type.
 */
export function zodValidate<Values>(schema: z.ZodType): (values: Values) => FormErrors {
  return zod4Resolver(schema) as unknown as (values: Values) => FormErrors;
}
