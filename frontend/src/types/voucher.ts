export interface Voucher {
  id: string;
  createdAt: string;
  name: string;
  code: string;
  authorizedGuestLimit?: number | null;
  authorizedGuestCount: number;
  activatedAt?: string | null;
  expiresAt?: string | null;
  expired: boolean;
  timeLimitMinutes: number;
  remarks: string;
}

export interface VoucherCreateData extends Omit<
  Voucher,
  | "id"
  | "createdAt"
  | "code"
  | "authorizedGuestCount"
  | "activatedAt"
  | "expiresAt"
  | "expired"
  | "remarks"
> {
  count: number;
  /** A custom key; only for a single voucher. */
  code?: string;
  remarks?: string;
}

export interface VoucherGetResponse {
  offset: number;
  limit: number;
  count: number;
  totalCount: number;
  data: Voucher[];
}

export interface VoucherDeletedResponse {
  vouchersDeleted: number;
}

export interface VoucherCreatedResponse {
  vouchers: Voucher[];
}
