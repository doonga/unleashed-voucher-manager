import {
  Voucher,
  VoucherCreateData,
  VoucherCreatedResponse,
  VoucherDeletedResponse,
  VoucherGetResponse,
} from "@/types/voucher";
import { notifyVouchersUpdated } from "./actions";

function removeNullUndefined<T extends Record<string, any>>(obj: T): T {
  return Object.fromEntries(
    Object.entries(obj).filter(
      ([_, value]) => value !== null && value !== undefined,
    ),
  ) as T;
}

async function call<T>(endpoint: string, opts: RequestInit = {}) {
  const res = await fetch(`/rust-api/${endpoint}`, {
    headers: { "Content-Type": "application/json" },
    ...opts,
  });
  if (!res.ok) {
    // The backend explains rejected requests in the response body.
    const message = (await res.text().catch(() => "")).trim();
    const error = new Error(message || res.statusText);
    (error as any).status = res.status;
    throw error;
  }
  return res.json() as Promise<T>;
}

export const MIN_VOUCHER_DURATION_HOURS = 1;
export const MAX_VOUCHER_DURATION_HOURS = 8_760;

export const MIN_VOUCHER_COUNT = 1;
export const MAX_VOUCHER_COUNT = 100;

export const MIN_VOUCHER_GUESTS = 1;
export const MAX_VOUCHER_GUESTS = 1000;

export { MIN_KEY_LENGTH as MIN_VOUCHER_KEY_LENGTH, MAX_KEY_LENGTH as MAX_VOUCHER_KEY_LENGTH } from "./validation";

export const api = {
  getAllVouchers: () => call<VoucherGetResponse>("/vouchers"),

  getRollingVoucher: () => call<Voucher>("/vouchers/rolling"),

  getNewestVoucher: () => call<Voucher>("/vouchers/newest"),

  getVoucherDetails: (id: string) =>
    call<Voucher>(`/vouchers/details?id=${encodeURIComponent(id)}`),

  createVoucher: async (data: VoucherCreateData) => {
    const filteredData = removeNullUndefined(data);
    const result = await call<VoucherCreatedResponse>("/vouchers", {
      method: "POST",
      body: JSON.stringify(filteredData),
    });
    await notifyVouchersUpdated();
    return result;
  },

  createRollingVoucher: async () => {
    const result = await call<Voucher>("/vouchers/rolling", {
      method: "POST",
    });
    await notifyVouchersUpdated();
    return result;
  },

  deleteExpiredVouchers: async () => {
    const result = await call<VoucherDeletedResponse>("/vouchers/expired", {
      method: "DELETE",
    });
    await notifyVouchersUpdated();
    return result;
  },

  deleteExpiredRollingVouchers: async () => {
    const result = await call<VoucherDeletedResponse>(
      "/vouchers/expired/rolling",
      {
        method: "DELETE",
      },
    );
    await notifyVouchersUpdated();
    return result;
  },

  deleteSelectedVouchers: async (ids: string[]) => {
    const BATCH_SIZE = 30;
    let totalDeleted = 0;

    try {
      for (let i = 0; i < ids.length; i += BATCH_SIZE) {
        const batch = ids.slice(i, i + BATCH_SIZE);
        const qs = batch.map(encodeURIComponent).join(",");
        const result: VoucherDeletedResponse =
          await call<VoucherDeletedResponse>(`/vouchers/selected?ids=${qs}`, {
            method: "DELETE",
          });
        totalDeleted += result.vouchersDeleted;
      }

      return {
        vouchersDeleted: totalDeleted,
      };
    } finally {
      await notifyVouchersUpdated();
    }
  },
};
