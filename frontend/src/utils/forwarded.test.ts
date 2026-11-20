import { test } from "node:test";
import assert from "node:assert/strict";
import { clientIpFromForwarded } from "./forwarded.ts";

test("uses the last forwarded entry", () => {
  assert.equal(clientIpFromForwarded("10.9.9.9, 192.168.50.7"), "192.168.50.7");
  assert.equal(clientIpFromForwarded("10.9.9.9,192.0.2.4 , 192.0.2.7"), "192.0.2.7");
});

test("single entry", () => {
  assert.equal(clientIpFromForwarded("192.0.2.7"), "192.0.2.7");
  assert.equal(clientIpFromForwarded("2001:db8::1"), "2001:db8::1");
});

test("strips IPv4-mapped prefix", () => {
  assert.equal(clientIpFromForwarded("::ffff:192.0.2.7"), "192.0.2.7");
});

test("rejects junk", () => {
  for (const value of ["hello", "", null, "192.0.2", "999.1.1.1", "10.9.9.9, nope"]) {
    assert.equal(clientIpFromForwarded(value), null, String(value));
  }
});
