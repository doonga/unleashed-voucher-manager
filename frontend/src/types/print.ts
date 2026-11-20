import { Voucher } from "./voucher";

export type PrintMode = "list" | "grid";

export type PrintJob = {
  vouchers: Voucher[];
  mode: PrintMode;
  createdAt: number;
};

export type PrintConfig = {
  showName: boolean;
  showLogo: boolean;
  showDuration: boolean;
  showMaxGuests: boolean;
  showId: boolean;
  showPrintTime: boolean;
};
