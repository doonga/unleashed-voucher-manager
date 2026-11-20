const IPV4_MAPPED_PREFIX = "::ffff:";
const IPV4 = /^(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}$/;
const IPV6 = /^[0-9a-f:]+$/i;

function isAddress(value: string): boolean {
  return IPV4.test(value) || (value.includes(":") && IPV6.test(value));
}

/**
 * The client's address from `X-Forwarded-For`: the last entry, which the
 * reverse proxy in front of the app appended. Earlier entries come from the
 * client and can be forged. Returns null when the entry is not an address.
 */
export function clientIpFromForwarded(header: string | null): string | null {
  if (!header) return null;
  let last = header.split(",").pop()?.trim() ?? "";
  if (last.toLowerCase().startsWith(IPV4_MAPPED_PREFIX) && IPV4.test(last.slice(IPV4_MAPPED_PREFIX.length))) {
    last = last.slice(IPV4_MAPPED_PREFIX.length);
  }
  return isAddress(last) ? last : null;
}
