"use client";

import SuccessModal from "@/components/modals/SuccessModal";
import { Voucher, VoucherCreateData } from "@/types/voucher";
import {
  api,
  MAX_VOUCHER_COUNT,
  MAX_VOUCHER_DURATION_HOURS,
  MAX_VOUCHER_GUESTS,
  MAX_VOUCHER_KEY_LENGTH,
  MIN_VOUCHER_COUNT,
  MIN_VOUCHER_GUESTS,
} from "@/utils/api";
import { map } from "@/utils/functional";
import { createErrorMessage } from "@/utils/createError";
import { notify } from "@/utils/notifications";
import { keyError, nameError, remarksError } from "@/utils/validation";
import { useCallback, useState, SubmitEvent } from "react";

// The controller only takes whole hours, so durations start at hours.
type TimeUnit = "hours" | "days" | "weeks";

const HOURS_PER_UNIT: Record<TimeUnit, number> = {
  hours: 1,
  days: 24,
  weeks: 168,
};

const DEFAULT_DURATION: Record<TimeUnit, number> = {
  hours: 24,
  days: 1,
  weeks: 1,
};

export default function CustomCreateTab() {
  const [loading, setLoading] = useState(false);
  const [newVouchers, setNewVouchers] = useState<Voucher[] | null>(null);
  const [durationUnit, setDurationUnit] = useState<TimeUnit>("days");
  // Kept as typed, so the field can be cleared and retyped; parsed on use.
  const [count, setCount] = useState(String(MIN_VOUCHER_COUNT));
  const isBatch = Number(count) > 1;

  const handleSubmit = async (e: SubmitEvent) => {
    e.preventDefault();

    const parseNumber = (x: FormDataEntryValue) =>
      x !== "" ? Number(x) : null;

    const form = e.currentTarget as HTMLFormElement;
    const data = new FormData(form);

    const rawDuration = Number(data.get("duration"));
    const unit = String(data.get("durationUnit") || "days") as TimeUnit;
    if (!Number.isFinite(rawDuration) || rawDuration <= 0) {
      notify("Duration must be a positive number", "error");
      return;
    }
    const durationHours = Math.ceil(rawDuration * HOURS_PER_UNIT[unit]);
    if (durationHours > MAX_VOUCHER_DURATION_HOURS) {
      notify(
        `Duration too long. Maximum allowed is ${MAX_VOUCHER_DURATION_HOURS} hours`,
        "error",
      );
      return;
    }

    // Disabled inputs are not in FormData: batches are named by the
    // controller and get generated keys.
    const name = String(data.get("name") ?? "");
    const code = String(data.get("code") ?? "").trim();
    const remarks = String(data.get("remarks") ?? "");
    const problem =
      nameError(name) ??
      (isBatch ? null : keyError(code) ?? remarksError(remarks));
    if (problem) {
      notify(problem, "error");
      return;
    }

    const payload: VoucherCreateData = {
      count: Number(data.get("count")),
      name,
      timeLimitMinutes: durationHours * 60,
      authorizedGuestLimit: map(data.get("guests"), parseNumber),
      code: code || undefined,
      remarks: (!isBatch && remarks) || undefined,
    };

    setLoading(true);
    try {
      const res = await api.createVoucher(payload);
      setNewVouchers(res.vouchers);
      notify(`Successfully created ${res.vouchers.length} vouchers`, "success");
      // reset() puts the unit select back to its default; keep the state
      // in step so the duration field matches it.
      form.reset();
      setCount(String(MIN_VOUCHER_COUNT));
      setDurationUnit("days");
    } catch (error) {
      notify(createErrorMessage(error), "error");
    }
    setLoading(false);
  };

  const closeModal = useCallback(() => {
    setNewVouchers(null);
  }, []);

  return (
    <div>
      <form onSubmit={handleSubmit} className="card max-w-lg mx-auto space-y-6">
        <div>
          <label className="block font-medium mb-1">Number</label>
          <input
            name="count"
            type="number"
            required
            min={MIN_VOUCHER_COUNT}
            max={MAX_VOUCHER_COUNT}
            value={count}
            onChange={(e) => setCount(e.target.value)}
          />
        </div>

        <div>
          <label className="block font-medium mb-1">Name</label>
          <input
            name="name"
            type="text"
            required={!isBatch}
            disabled={isBatch}
            defaultValue="Custom Voucher"
          />
          {isBatch && (
            <p className="text-sm text-muted mt-1">
              The controller names batch vouchers Guest-1, Guest-2 and so on.
            </p>
          )}
        </div>

        <div>
          <label className="block font-medium mb-1">Duration</label>
          <div className="flex-center gap-2">
            <input
              key={durationUnit}
              name="duration"
              type="number"
              required
              min={1}
              max={Math.floor(MAX_VOUCHER_DURATION_HOURS / HOURS_PER_UNIT[durationUnit])}
              defaultValue={DEFAULT_DURATION[durationUnit]}
            />
            <select
              name="durationUnit"
              onChange={(e) => setDurationUnit(e.target.value as TimeUnit)}
              className="w-auto"
              value={durationUnit}
            >
              <option value="hours">Hours</option>
              <option value="days">Days</option>
              <option value="weeks">Weeks</option>
            </select>
          </div>
        </div>

        <div>
          <label className="block font-medium mb-1">Guest Limit</label>
          <input
            name="guests"
            type="number"
            min={MIN_VOUCHER_GUESTS}
            max={MAX_VOUCHER_GUESTS}
            placeholder="Unlimited"
          />
        </div>

        <div>
          <label className="block font-medium mb-1">Key</label>
          <input
            name="code"
            type="text"
            disabled={isBatch}
            maxLength={MAX_VOUCHER_KEY_LENGTH}
            placeholder={isBatch ? "Generated for batches" : "Generated"}
            autoComplete="off"
          />
        </div>

        <div>
          <label className="block font-medium mb-1">Remarks</label>
          <input
            name="remarks"
            type="text"
            disabled={isBatch}
            placeholder={isBatch ? "Set by the controller for batches" : "Optional"}
          />
        </div>

        <button type="submit" disabled={loading} className="btn-primary w-full">
          {loading ? "Creating…" : "Create Custom Voucher"}
        </button>
      </form>
      {newVouchers && (
        <SuccessModal vouchers={newVouchers} onClose={closeModal} />
      )}
    </div>
  );
}
