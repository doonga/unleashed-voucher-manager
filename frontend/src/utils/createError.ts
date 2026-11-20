const GENERIC = "Failed to create voucher";

/**
 * What to tell staff when creating vouchers fails. The backend explains a
 * rejected request (400) in the response body, which the API helper puts in
 * the error message.
 */
export function createErrorMessage(error: unknown): string {
  if (!(error instanceof Error)) return GENERIC;
  const status = (error as { status?: number }).status;
  if (status === 409) return "That key is already in use";
  if (status === 400 && error.message.trim()) return error.message.trim();
  return GENERIC;
}
