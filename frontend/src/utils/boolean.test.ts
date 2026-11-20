import { test } from "node:test";
import assert from "node:assert/strict";
import { parseBoolean } from "./boolean.ts";

test("environment strings become booleans", () => {
  for (const value of ["true", "TRUE", " yes ", "1", "on"]) {
    assert.equal(parseBoolean(value, false), true, value);
  }
  for (const value of ["false", "False", "0", "no", " off "]) {
    assert.equal(parseBoolean(value, true), false, value);
  }
});

test("unreadable values use the fallback", () => {
  for (const value of [undefined, null, "", "maybe", 2, {}]) {
    assert.equal(parseBoolean(value, true), true);
    assert.equal(parseBoolean(value, false), false);
  }
});

test("real booleans pass through", () => {
  assert.equal(parseBoolean(true, false), true);
  assert.equal(parseBoolean(false, true), false);
});
