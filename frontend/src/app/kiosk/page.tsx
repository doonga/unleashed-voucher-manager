"use client";

import { useCallback, useEffect, useState } from "react";
import Spinner from "@/components/utils/Spinner";
import WifiQr from "@/components/utils/WifiQr";
import { TriState } from "@/types/state";
import { Voucher } from "@/types/voucher";
import { api } from "@/utils/api";
import { formatCode } from "@/utils/format";
import { useGlobal } from "@/contexts/GlobalContext";

const REFRESH_INTERVAL_MS = 60_000;

export default function KioskPage() {
  const [voucher, setVoucher] = useState<Voucher | null>(null);
  const [state, setState] = useState<TriState | null>(null);
  const { wifiConfig, wifiString } = useGlobal();

  // `quiet` refreshes keep the current voucher on screen instead of showing
  // the spinner.
  const load = useCallback(async (quiet = false) => {
    try {
      if (!quiet) setState("loading");
      await api.getRollingVoucher().then(setVoucher);
      setState("ok");
    } catch (error: any) {
      if (error?.status !== 404) {
        setState("error");
        return;
      }
      try {
        await api.createRollingVoucher().then(setVoucher);
        setState("ok");
      } catch {
        setState("error");
      }
    }
  }, []);

  useEffect(() => {
    load();
    const refresh = () => load(true);
    window.addEventListener("vouchersUpdated", refresh);
    // An unused voucher still counts down on the controller and disappears
    // when it expires, and a guest can use it without /welcome loading, so
    // check every minute and create the next one when none is waiting.
    const timer = window.setInterval(refresh, REFRESH_INTERVAL_MS);
    return () => {
      window.removeEventListener("vouchersUpdated", refresh);
      window.clearInterval(timer);
    };
  }, [load]);

  const renderContent = useCallback(() => {
    switch (state) {
      case null:
      case "loading":
        return <Spinner />;
      case "error":
        return (
          <div className="text-center text-5xl sm:text-6xl md:text-7xl text-status-danger">
            Could not load rolling voucher
          </div>
        );
      case "ok":
        const qrAvailable = wifiConfig && wifiString;
        return (
          <div
            className={`grid grid-cols-1 ${qrAvailable && "md:grid-cols-2 "} gap-8 items-center`}
          >
            {qrAvailable && <WifiQr className="w-full sm:h-80 md:h-96 " />}
            <div className={`text-center ${qrAvailable && "md:text-left"}`}>
              <h2 className="font-medium mb-4 text-3xl sm:text-4xl md:text-5xl">
                Voucher Code
              </h2>
              <div className="voucher-code tracking-widest text-5xl sm:text-6xl md:text-7xl">
                {voucher ? formatCode(voucher.code) : "No voucher available"}
              </div>
            </div>
          </div>
        );
    }
  }, [voucher, state, wifiConfig, wifiString]);

  return (
    <main className="flex-center h-screen w-full px-4">{renderContent()}</main>
  );
}
