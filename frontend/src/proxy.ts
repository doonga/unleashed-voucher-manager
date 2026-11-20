import { NextResponse, NextRequest } from "next/server";
import { isInBlockedSubnet } from "@/utils/ipv4";
import { clientIpFromForwarded } from "@/utils/forwarded";

export const config = {
  matcher: ["/", "/rust-api/:path*"],
};

const DEFAULT_FRONTEND_TO_BACKEND_URL = "http://127.0.0.1";
const DEFAULT_BACKEND_BIND_PORT = "8080";

const guestAllowedPaths = [
  "/welcome",
  "/rust-api/vouchers/rolling",
  "favicon.ico",
  "favicon.svg",
];

export function proxy(request: NextRequest) {
  const { pathname } = request.nextUrl;

  // The client's address is the last X-Forwarded-For entry, the one the
  // reverse proxy in front of the app appended; earlier entries can be forged.
  const clientIp =
    clientIpFromForwarded(request.headers.get("x-forwarded-for")) ?? "";

  // Restrict access based on GUEST_SUBNETWORK env variable
  const guestSubnet = process.env.GUEST_SUBNETWORK;
  if (guestSubnet) {
    if (
      !guestAllowedPaths.includes(pathname) &&
      isInBlockedSubnet(clientIp, guestSubnet)
    ) {
      return new NextResponse("Access denied", { status: 403 });
    }
  }

  if (pathname.startsWith("/rust-api")) {
    // Remove the /rust-api prefix and reconstruct the path for the backend
    const backendPath = request.nextUrl.pathname.replace(/^\/rust-api/, "/api");

    const backendUrl =
      process.env.FRONTEND_TO_BACKEND_URL || DEFAULT_FRONTEND_TO_BACKEND_URL;
    const backendPort =
      process.env.BACKEND_BIND_PORT || DEFAULT_BACKEND_BIND_PORT;

    const backendFullUrl = new URL(
      `${backendUrl}:${backendPort}${backendPath}${request.nextUrl.search}`,
    );

    return NextResponse.rewrite(backendFullUrl, { request });
  }

  return NextResponse.next();
}
